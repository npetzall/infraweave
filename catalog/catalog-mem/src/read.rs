//! [`CatalogRead`](catalog_trait::CatalogRead) for [`MemCatalog`](crate::MemCatalog).
//!
//! ## `VersionSelector::Latest`
//!
//! Among entries sharing `(kind, name, track)`, **Latest** picks the row with the greatest
//! [`metadata.version`](catalog_trait::types::Metadata::version) using [`crate::store::cmp_version_str`]:
//! valid semver strings compare as semver; non-semver strings sort *before* any semver and compare
//! lexicographically with each other.
//!
//! ## Projection (`Query::projection`)
//!
//! - `None` means **full** response: all [`ProjectionFields`] bits are applied (same as
//!   [`ProjectionFields::ALL`]).
//! - `Some(mask)` populates only fields whose bits are set: `metadata`, `manifest`, `terraform`,
//!   and for stacks [`ProjectionFields::STACK_DATA`] (`stack_data`). The in-memory store does not
//!   persist `provider_mirror`; that field stays unset here even when `PROVIDER_MIRROR` is requested.
//!
//! [`get`](catalog_trait::CatalogRead::get) has no `Query` parameter; it always returns **full**
//! projection for that entry (HTTP layers that need a partial `get` should use
//! [`list`](catalog_trait::CatalogRead::list) with filters or a dedicated API).
//!
//! ## Pagination (`Query::next` / `Page::next`)
//!
//! After filters and sorting, [`Query::limit`](catalog_trait::read::Query::limit) slices the result set.
//! [`Page::next`](catalog_trait::read::Page::next) is an **opaque** base64 (standard) JSON cursor
//! (`ListCursorV1 { offset }`) into that sorted list for this [`MemCatalog`](crate::MemCatalog)
//! instance. Tokens are not portable across instances or durable catalog mutations that change list
//! ordering or length in ways that invalidate the offset.
//!
//! Malformed tokens, JSON that does not match the schema, or an `offset` past the end of the
//! filtered list yield a clear [`anyhow`] error (same message prefix: `invalid catalog-mem list
//! continuation token`).
//!
//! ## Yanked entries
//!
//! Rows marked **yanked** in the store are omitted from [`list`](catalog_trait::CatalogRead::list) and
//! [`get`](catalog_trait::CatalogRead::get) (including [`VersionSelector::Latest`](catalog_trait::types::VersionSelector::Latest)).
//! [`download_*`](catalog_trait::CatalogRead::download_provider) and attachment APIs error if the id refers
//! to a yanked row.

use async_trait::async_trait;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use catalog_trait::read::{
    CatalogEntry, ContentSource, Module, Page, ProjectionFields, Provider, Query, Stack,
};
use catalog_trait::types::{CatalogKind, CatalogRef, VersionSelector};
use catalog_trait::CatalogRead;

use crate::store::{cmp_version_str, KindPayload, StoredEntry};
use crate::MemCatalog;

fn sort_list_entries(mut items: Vec<StoredEntry>) -> Vec<StoredEntry> {
    items.sort_by(|a, b| {
        a.metadata
            .name
            .cmp(&b.metadata.name)
            .then_with(|| a.metadata.track.cmp(&b.metadata.track))
            .then_with(|| cmp_version_str(&a.metadata.version, &b.metadata.version))
    });
    items
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct ListCursorV1 {
    offset: usize,
}

fn encode_list_cursor(offset: usize) -> String {
    let json = serde_json::to_vec(&ListCursorV1 { offset }).expect("list cursor JSON");
    B64.encode(json)
}

fn decode_list_cursor(token: &str) -> anyhow::Result<usize> {
    let bytes = B64
        .decode(token.trim().as_bytes())
        .map_err(|_| anyhow::anyhow!("invalid catalog-mem list continuation token"))?;
    let c: ListCursorV1 = serde_json::from_slice(&bytes)
        .map_err(|_| anyhow::anyhow!("invalid catalog-mem list continuation token"))?;
    Ok(c.offset)
}

fn stored_to_provider(entry: &StoredEntry, mask: ProjectionFields) -> Provider {
    let mut p = Provider::new(entry.reference.clone());
    if mask.contains(ProjectionFields::METADATA) {
        p.metadata = Some(entry.metadata.clone());
    }
    if mask.contains(ProjectionFields::MANIFEST) {
        if let KindPayload::Provider(m) = &entry.kind_payload {
            p.manifest = Some(m.clone());
        }
    }
    if mask.contains(ProjectionFields::TERRAFORM) {
        p.terraform = Some(entry.terraform.clone());
    }
    p
}

fn stored_to_module(entry: &StoredEntry, mask: ProjectionFields) -> Module {
    let mut m = Module::new(entry.reference.clone());
    if mask.contains(ProjectionFields::METADATA) {
        m.metadata = Some(entry.metadata.clone());
    }
    if mask.contains(ProjectionFields::MANIFEST) {
        if let KindPayload::Module(manifest) = &entry.kind_payload {
            m.manifest = Some(manifest.clone());
        }
    }
    if mask.contains(ProjectionFields::TERRAFORM) {
        m.terraform = Some(entry.terraform.clone());
    }
    m
}

fn stored_to_stack(entry: &StoredEntry, mask: ProjectionFields) -> Stack {
    let mut s = Stack::new(entry.reference.clone());
    if mask.contains(ProjectionFields::METADATA) {
        s.metadata = Some(entry.metadata.clone());
    }
    if mask.contains(ProjectionFields::MANIFEST) {
        if let KindPayload::Stack { manifest, .. } = &entry.kind_payload {
            s.manifest = Some(manifest.clone());
        }
    }
    if mask.contains(ProjectionFields::TERRAFORM) {
        s.terraform = Some(entry.terraform.clone());
    }
    if mask.contains(ProjectionFields::STACK_DATA) {
        if let KindPayload::Stack { stack_data, .. } = &entry.kind_payload {
            s.stack_data = stack_data.clone();
        }
    }
    s
}

fn stored_to_catalog_entry(
    entry: &StoredEntry,
    projection: Option<ProjectionFields>,
) -> CatalogEntry {
    let mask = projection.unwrap_or(ProjectionFields::ALL);
    match entry.kind {
        CatalogKind::Provider => CatalogEntry::Provider(stored_to_provider(entry, mask)),
        CatalogKind::Module => CatalogEntry::Module(stored_to_module(entry, mask)),
        CatalogKind::Stack => CatalogEntry::Stack(stored_to_stack(entry, mask)),
    }
}

#[async_trait]
impl CatalogRead for MemCatalog {
    async fn list(&self, kind: CatalogKind, query: &Query) -> anyhow::Result<Page<CatalogEntry>> {
        let mut items =
            self.store
                .list_filtered(kind, query.name.as_deref(), query.track.as_deref());
        items = sort_list_entries(items);

        let offset = match query.next.as_deref() {
            Some(t) => decode_list_cursor(t)?,
            None => 0,
        };
        if offset > items.len() {
            anyhow::bail!("invalid catalog-mem list continuation token");
        }

        let limit = query.limit.map(|l| l as usize).unwrap_or(usize::MAX);
        let end = (offset + limit).min(items.len());
        let page_items: Vec<CatalogEntry> = items[offset..end]
            .iter()
            .map(|e| stored_to_catalog_entry(e, query.projection))
            .collect();
        let next = if end < items.len() {
            Some(encode_list_cursor(end))
        } else {
            None
        };
        Ok(Page {
            items: page_items,
            next,
        })
    }

    async fn get(
        &self,
        kind: CatalogKind,
        name: &str,
        track: &str,
        version: VersionSelector,
    ) -> anyhow::Result<Option<CatalogEntry>> {
        let entry = self.store.resolve_version(kind, name, track, &version);
        Ok(entry.map(|e| stored_to_catalog_entry(&e, None)))
    }

    async fn download_provider(&self, reference: &CatalogRef) -> anyhow::Result<ContentSource> {
        let entry = self
            .store
            .get_entry(&reference.id)
            .ok_or_else(|| anyhow::anyhow!("unknown catalog id: {}", reference.id))?;
        if entry.yanked {
            anyhow::bail!("catalog id {} is yanked", reference.id);
        }
        if entry.kind != CatalogKind::Provider {
            anyhow::bail!(
                "catalog id {} is {:?}, not a provider",
                reference.id,
                entry.kind
            );
        }
        Ok(ContentSource::Bytes(entry.content.clone()))
    }

    async fn download_module(&self, reference: &CatalogRef) -> anyhow::Result<ContentSource> {
        let entry = self
            .store
            .get_entry(&reference.id)
            .ok_or_else(|| anyhow::anyhow!("unknown catalog id: {}", reference.id))?;
        if entry.yanked {
            anyhow::bail!("catalog id {} is yanked", reference.id);
        }
        if entry.kind != CatalogKind::Module {
            anyhow::bail!(
                "catalog id {} is {:?}, not a module",
                reference.id,
                entry.kind
            );
        }
        Ok(ContentSource::Bytes(entry.content.clone()))
    }

    async fn download_stack(&self, reference: &CatalogRef) -> anyhow::Result<ContentSource> {
        let entry = self
            .store
            .get_entry(&reference.id)
            .ok_or_else(|| anyhow::anyhow!("unknown catalog id: {}", reference.id))?;
        if entry.yanked {
            anyhow::bail!("catalog id {} is yanked", reference.id);
        }
        if entry.kind != CatalogKind::Stack {
            anyhow::bail!(
                "catalog id {} is {:?}, not a stack",
                reference.id,
                entry.kind
            );
        }
        Ok(ContentSource::Bytes(entry.content.clone()))
    }

    async fn list_attachments(&self, reference: &CatalogRef) -> anyhow::Result<Vec<String>> {
        let entry = self
            .store
            .get_entry(&reference.id)
            .ok_or_else(|| anyhow::anyhow!("unknown catalog id: {}", reference.id))?;
        if entry.yanked {
            anyhow::bail!("catalog id {} is yanked", reference.id);
        }
        let mut names: Vec<String> = entry.attachments.keys().cloned().collect();
        names.sort();
        Ok(names)
    }

    async fn download_attachment(
        &self,
        reference: &CatalogRef,
        name: &str,
    ) -> anyhow::Result<ContentSource> {
        let entry = self
            .store
            .get_entry(&reference.id)
            .ok_or_else(|| anyhow::anyhow!("unknown catalog id: {}", reference.id))?;
        if entry.yanked {
            anyhow::bail!("catalog id {} is yanked", reference.id);
        }
        let bytes = entry
            .attachments
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("unknown attachment: {name}"))?;
        Ok(ContentSource::Bytes(bytes.clone()))
    }
}
