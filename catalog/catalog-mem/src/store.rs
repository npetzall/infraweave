//! Internal storage for [`super::MemCatalog`](crate::MemCatalog).
//!
//! Rows are keyed by a stable string id (UUID v4) stored in [`CatalogRef`](catalog_trait::types::CatalogRef).
//! A secondary index maps `(kind, name, track)` to version ids for
//! [`VersionSelector::Latest`](catalog_trait::types::VersionSelector) resolution (highest parseable semver;
//! non-semver strings sort before any valid semver and compare lexicographically with each other).

use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::RwLock;

use catalog_trait::types::{
    CatalogKind, CatalogRef, Metadata, TerraformInterface, VersionSelector,
};
use catalog_trait::{ModuleManifest, ModuleStackData, ProviderManifest, StackManifest};

fn logical_key(kind: CatalogKind, name: &str, track: &str) -> String {
    let tag = match kind {
        CatalogKind::Provider => "provider",
        CatalogKind::Module => "module",
        CatalogKind::Stack => "stack",
    };
    // Unambiguous separator (name/track may contain `:`).
    format!("{tag}\x1f{name}\x1f{track}")
}

/// Kind-specific manifest and optional stack-only fields.
#[derive(Debug, Clone)]
pub(crate) enum KindPayload {
    Provider(ProviderManifest),
    Module(ModuleManifest),
    Stack {
        manifest: StackManifest,
        stack_data: Option<ModuleStackData>,
    },
}

/// One stored catalog version: metadata, payloads, primary binary, attachments.
#[derive(Debug, Clone)]
pub(crate) struct StoredEntry {
    pub kind: CatalogKind,
    pub reference: CatalogRef,
    pub metadata: Metadata,
    /// When true, the row is hidden from list/get/Latest and blocks downloads (see [`Store::yank`]).
    pub yanked: bool,
    pub terraform: TerraformInterface,
    pub kind_payload: KindPayload,
    pub content: Vec<u8>,
    pub attachments: HashMap<String, Vec<u8>>,
}

/// Thread-safe in-memory maps: entries by id, and ids grouped by logical `(kind, name, track)`.
#[derive(Debug, Default)]
pub(crate) struct Store {
    entries: RwLock<HashMap<String, StoredEntry>>,
    /// Insertion order per logical key; `Latest` uses semver ordering over [`StoredEntry::metadata`].
    by_logical: RwLock<HashMap<String, Vec<String>>>,
}

/// Total ordering for version strings: valid semver compares as semver; invalid sorts before valid
/// and compares lexicographically among themselves (matches typical “prerelease” handling needs).
pub(crate) fn cmp_version_str(a: &str, b: &str) -> Ordering {
    match (semver::Version::parse(a), semver::Version::parse(b)) {
        (Ok(va), Ok(vb)) => va.cmp(&vb),
        (Ok(_), Err(_)) => Ordering::Greater,
        (Err(_), Ok(_)) => Ordering::Less,
        (Err(_), Err(_)) => a.cmp(b),
    }
}

impl Store {
    pub(crate) fn get_entry(&self, id: &str) -> Option<StoredEntry> {
        let g = self.entries.read().ok()?;
        g.get(id).cloned()
    }

    fn insert_inner(&self, entry: StoredEntry) -> CatalogRef {
        let id = entry.reference.id.clone();
        let key = logical_key(entry.kind, &entry.metadata.name, &entry.metadata.track);

        {
            let mut g = self
                .entries
                .write()
                .expect("catalog-mem store lock poisoned");
            g.insert(id.clone(), entry);
        }
        {
            let mut idx = self
                .by_logical
                .write()
                .expect("catalog-mem index lock poisoned");
            idx.entry(key).or_default().push(id.clone());
        }

        CatalogRef { id }
    }

    pub(crate) fn insert_provider(
        &self,
        metadata: Metadata,
        manifest: ProviderManifest,
        terraform: TerraformInterface,
        content: Vec<u8>,
    ) -> CatalogRef {
        let id = uuid::Uuid::new_v4().to_string();
        let reference = CatalogRef { id: id.clone() };
        let entry = StoredEntry {
            kind: CatalogKind::Provider,
            reference,
            metadata,
            yanked: false,
            terraform,
            kind_payload: KindPayload::Provider(manifest),
            content,
            attachments: HashMap::new(),
        };
        self.insert_inner(entry)
    }

    pub(crate) fn insert_module(
        &self,
        metadata: Metadata,
        manifest: ModuleManifest,
        terraform: TerraformInterface,
        content: Vec<u8>,
    ) -> CatalogRef {
        let id = uuid::Uuid::new_v4().to_string();
        let reference = CatalogRef { id: id.clone() };
        let entry = StoredEntry {
            kind: CatalogKind::Module,
            reference,
            metadata,
            yanked: false,
            terraform,
            kind_payload: KindPayload::Module(manifest),
            content,
            attachments: HashMap::new(),
        };
        self.insert_inner(entry)
    }

    pub(crate) fn insert_stack(
        &self,
        metadata: Metadata,
        manifest: StackManifest,
        terraform: TerraformInterface,
        stack_data: Option<ModuleStackData>,
        content: Vec<u8>,
    ) -> CatalogRef {
        let id = uuid::Uuid::new_v4().to_string();
        let reference = CatalogRef { id: id.clone() };
        let entry = StoredEntry {
            kind: CatalogKind::Stack,
            reference,
            metadata,
            yanked: false,
            terraform,
            kind_payload: KindPayload::Stack {
                manifest,
                stack_data,
            },
            content,
            attachments: HashMap::new(),
        };
        self.insert_inner(entry)
    }

    pub(crate) fn insert_attachment(
        &self,
        catalog_id: &str,
        name: &str,
        bytes: Vec<u8>,
    ) -> anyhow::Result<()> {
        let mut g = self
            .entries
            .write()
            .expect("catalog-mem store lock poisoned");
        let entry = g
            .get_mut(catalog_id)
            .ok_or_else(|| anyhow::anyhow!("unknown catalog id: {catalog_id}"))?;
        if entry.yanked {
            anyhow::bail!("cannot add attachment to yanked catalog id: {catalog_id}");
        }
        entry.attachments.insert(name.to_string(), bytes);
        Ok(())
    }

    pub(crate) fn promote(
        &self,
        kind: CatalogKind,
        reference: &CatalogRef,
        track: &str,
        version: Option<&str>,
    ) -> anyhow::Result<()> {
        let new_track = track.to_string();
        let mut entries = self
            .entries
            .write()
            .expect("catalog-mem store lock poisoned");
        let entry = entries
            .get_mut(&reference.id)
            .ok_or_else(|| anyhow::anyhow!("unknown catalog id: {}", reference.id))?;
        if entry.kind != kind {
            anyhow::bail!(
                "catalog id {} is {:?}, expected {:?}",
                reference.id,
                entry.kind,
                kind
            );
        }
        if entry.yanked {
            anyhow::bail!("cannot promote a yanked catalog entry");
        }
        let name = entry.metadata.name.clone();
        let old_track = entry.metadata.track.clone();
        let old_version = entry.metadata.version.clone();
        let new_version = version
            .map(str::to_string)
            .unwrap_or_else(|| old_version.clone());

        if old_track == new_track && old_version == new_version {
            return Ok(());
        }

        for (id, e) in entries.iter() {
            if id == &reference.id {
                continue;
            }
            if e.kind == kind
                && e.metadata.name == name
                && e.metadata.track == new_track
                && e.metadata.version == new_version
            {
                anyhow::bail!(
                    "promote blocked: {:?} {}@{} {} already exists",
                    kind,
                    name,
                    new_track,
                    new_version
                );
            }
        }

        let old_key = logical_key(kind, &name, &old_track);
        let new_key = logical_key(kind, &name, &new_track);
        if old_key != new_key {
            let mut idx = self
                .by_logical
                .write()
                .expect("catalog-mem index lock poisoned");
            if let Some(vec) = idx.get_mut(&old_key) {
                vec.retain(|id| id != &reference.id);
            }
            idx.entry(new_key).or_default().push(reference.id.clone());
        }

        let entry = entries
            .get_mut(&reference.id)
            .expect("entry still present after index update");
        entry.metadata.track = new_track;
        entry.metadata.version = new_version;
        Ok(())
    }

    pub(crate) fn deprecate(
        &self,
        kind: CatalogKind,
        reference: &CatalogRef,
        reason: &str,
    ) -> anyhow::Result<()> {
        let mut entries = self
            .entries
            .write()
            .expect("catalog-mem store lock poisoned");
        let entry = entries
            .get_mut(&reference.id)
            .ok_or_else(|| anyhow::anyhow!("unknown catalog id: {}", reference.id))?;
        if entry.kind != kind {
            anyhow::bail!(
                "catalog id {} is {:?}, expected {:?}",
                reference.id,
                entry.kind,
                kind
            );
        }
        if entry.yanked {
            anyhow::bail!("cannot deprecate a yanked catalog entry");
        }
        entry.metadata.deprecated = true;
        entry.metadata.deprecated_message = Some(reason.to_string());
        Ok(())
    }

    pub(crate) fn yank(&self, kind: CatalogKind, reference: &CatalogRef) -> anyhow::Result<()> {
        let mut entries = self
            .entries
            .write()
            .expect("catalog-mem store lock poisoned");
        let entry = entries
            .get_mut(&reference.id)
            .ok_or_else(|| anyhow::anyhow!("unknown catalog id: {}", reference.id))?;
        if entry.kind != kind {
            anyhow::bail!(
                "catalog id {} is {:?}, expected {:?}",
                reference.id,
                entry.kind,
                kind
            );
        }
        if entry.yanked {
            return Ok(());
        }
        let key = logical_key(kind, &entry.metadata.name, &entry.metadata.track);
        {
            let mut idx = self
                .by_logical
                .write()
                .expect("catalog-mem index lock poisoned");
            if let Some(vec) = idx.get_mut(&key) {
                vec.retain(|id| id != &reference.id);
            }
        }
        entry.yanked = true;
        Ok(())
    }

    pub(crate) fn logical_version_ids(
        &self,
        kind: CatalogKind,
        name: &str,
        track: &str,
    ) -> Vec<String> {
        let idx = self
            .by_logical
            .read()
            .expect("catalog-mem index lock poisoned");
        idx.get(&logical_key(kind, name, track))
            .cloned()
            .unwrap_or_default()
    }

    /// All stored rows for `kind`, optionally narrowed by metadata `name` / `track`.
    pub(crate) fn list_filtered(
        &self,
        kind: CatalogKind,
        name: Option<&str>,
        track: Option<&str>,
    ) -> Vec<StoredEntry> {
        let g = self
            .entries
            .read()
            .expect("catalog-mem store lock poisoned");
        g.values()
            .filter(|e| {
                !e.yanked
                    && e.kind == kind
                    && name.map_or(true, |n| e.metadata.name == n)
                    && track.map_or(true, |t| e.metadata.track == t)
            })
            .cloned()
            .collect()
    }

    pub(crate) fn resolve_version(
        &self,
        kind: CatalogKind,
        name: &str,
        track: &str,
        version: &VersionSelector,
    ) -> Option<StoredEntry> {
        let ids = self.logical_version_ids(kind, name, track);
        let entries: Vec<StoredEntry> = ids
            .iter()
            .filter_map(|id| self.get_entry(id))
            .filter(|e| e.kind == kind && !e.yanked)
            .collect();
        match version {
            VersionSelector::Exact(v) => entries.into_iter().find(|e| e.metadata.version == *v),
            VersionSelector::Latest => entries
                .into_iter()
                .max_by(|a, b| cmp_version_str(&a.metadata.version, &b.metadata.version)),
        }
    }
}
