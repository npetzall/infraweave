use async_trait::async_trait;
use env_defs::{ModuleManifest, ModuleStackData, ProviderManifest, StackManifest};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign};
use std::path::PathBuf;

pub use crate::types::{CatalogKind, CatalogRef, Metadata, TerraformInterface, VersionSelector};

// --- Types used only by CatalogRead ---

/// Where to read or fetch the binary content for catalog entries.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ContentSource {
    Url(String),
    Path(PathBuf),
    Bytes(Vec<u8>),
}

/// Bitmask-like selector for which heavy fields should be populated in list responses.
///
/// - `Query.projection == None` means "Full" (all supported projected fields should be populated).
/// - `Query.projection == Some(mask)` means "Only populate fields included in `mask`".
///
/// Includes `PROVIDER_MIRROR` for module/stack `provider_mirror` maps when the backend stores them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ProjectionFields {
    bits: u8,
}

impl ProjectionFields {
    const METADATA_BIT: u8 = 0b0001;
    const MANIFEST_BIT: u8 = 0b0010;
    const TERRAFORM_BIT: u8 = 0b0100;
    const PROVIDER_MIRROR_BIT: u8 = 0b1000;
    const STACK_DATA_BIT: u8 = 0b1_0000;

    pub const METADATA: ProjectionFields = ProjectionFields {
        bits: Self::METADATA_BIT,
    };
    pub const MANIFEST: ProjectionFields = ProjectionFields {
        bits: Self::MANIFEST_BIT,
    };
    pub const TERRAFORM: ProjectionFields = ProjectionFields {
        bits: Self::TERRAFORM_BIT,
    };
    pub const PROVIDER_MIRROR: ProjectionFields = ProjectionFields {
        bits: Self::PROVIDER_MIRROR_BIT,
    };
    pub const STACK_DATA: ProjectionFields = ProjectionFields {
        bits: Self::STACK_DATA_BIT,
    };
    pub const ALL: ProjectionFields = ProjectionFields {
        bits: Self::METADATA_BIT
            | Self::MANIFEST_BIT
            | Self::TERRAFORM_BIT
            | Self::PROVIDER_MIRROR_BIT
            | Self::STACK_DATA_BIT,
    };

    pub fn contains(self, flag: ProjectionFields) -> bool {
        (self.bits & flag.bits) != 0
    }
}

impl BitOr for ProjectionFields {
    type Output = ProjectionFields;
    fn bitor(self, rhs: ProjectionFields) -> Self::Output {
        ProjectionFields {
            bits: self.bits | rhs.bits,
        }
    }
}
impl BitOrAssign for ProjectionFields {
    fn bitor_assign(&mut self, rhs: ProjectionFields) {
        self.bits |= rhs.bits;
    }
}
impl BitAnd for ProjectionFields {
    type Output = ProjectionFields;
    fn bitand(self, rhs: ProjectionFields) -> Self::Output {
        ProjectionFields {
            bits: self.bits & rhs.bits,
        }
    }
}
impl BitAndAssign for ProjectionFields {
    fn bitand_assign(&mut self, rhs: ProjectionFields) {
        self.bits &= rhs.bits;
    }
}

/// Full provider entry as returned from queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    pub reference: CatalogRef,
    pub metadata: Option<Metadata>,
    pub manifest: Option<ProviderManifest>,
    pub terraform: Option<TerraformInterface>,
}

impl Provider {
    pub fn new(reference: CatalogRef) -> Self {
        Self {
            reference,
            metadata: None,
            manifest: None,
            terraform: None,
        }
    }
}

/// Full module entry as returned from queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Module {
    pub reference: CatalogRef,
    pub metadata: Option<Metadata>,
    pub manifest: Option<ModuleManifest>,
    pub terraform: Option<TerraformInterface>,
    /// Relative mirror destinations (`/` in JSON) → artifact source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_mirror: Option<HashMap<PathBuf, ContentSource>>,
}

impl Module {
    pub fn new(reference: CatalogRef) -> Self {
        Self {
            reference,
            metadata: None,
            manifest: None,
            terraform: None,
            provider_mirror: None,
        }
    }
}

/// Full stack entry as returned from queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stack {
    pub reference: CatalogRef,
    pub metadata: Option<Metadata>,
    pub manifest: Option<StackManifest>,
    pub terraform: Option<TerraformInterface>,
    pub stack_data: Option<ModuleStackData>,
    /// Relative mirror destinations (`/` in JSON) → artifact source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_mirror: Option<HashMap<PathBuf, ContentSource>>,
}

impl Stack {
    pub fn new(reference: CatalogRef) -> Self {
        Self {
            reference,
            metadata: None,
            manifest: None,
            terraform: None,
            stack_data: None,
            provider_mirror: None,
        }
    }
}

/// Page-enveloped response returned by all `list*` pagination APIs.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(bound(
    serialize = "T: serde::Serialize",
    deserialize = "T: serde::Deserialize<'de>"
))]
pub struct Page<T> {
    pub items: Vec<T>,
    pub next: Option<String>,
}

/// Unified query used for all list operations.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Query {
    pub name: Option<String>,
    pub track: Option<String>,
    pub limit: Option<u32>,
    pub next: Option<String>,
    pub projection: Option<ProjectionFields>,
}

/// Unified representation of a catalog entry when listing generically.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CatalogEntry {
    Provider(Provider),
    Module(Module),
    Stack(Stack),
}

// --- Trait ---

/// Read/query capability surface for providers, modules and stacks.
///
/// This trait models listing/fetching/downloading and attachment reads.
/// Write/population and lifecycle operations are defined in
/// `CatalogPopulate` and `CatalogManagement`.
#[async_trait]
pub trait CatalogRead: Send + Sync {
    //
    // Listing / queries
    //

    /// Unified listing entrypoint for all catalog kinds.
    ///
    /// Implementors can override this to handle all list queries in one place.
    async fn list(&self, kind: CatalogKind, query: &Query) -> anyhow::Result<Page<CatalogEntry>>;

    /// List providers matching the given query.
    ///
    /// By default, this routes through `list` and filters provider entries.
    async fn list_providers(&self, query: &Query) -> anyhow::Result<Page<Provider>> {
        let entries = self.list(CatalogKind::Provider, query).await?;
        let mut items = Vec::with_capacity(entries.items.len());
        for entry in entries.items {
            match entry {
                CatalogEntry::Provider(p) => items.push(p),
                _ => anyhow::bail!("list(Provider, ..) returned a non-provider catalog entry"),
            }
        }
        Ok(Page {
            items,
            next: entries.next,
        })
    }

    /// List modules matching the given query.
    ///
    /// By default, this routes through `list` and filters module entries.
    async fn list_modules(&self, query: &Query) -> anyhow::Result<Page<Module>> {
        let entries = self.list(CatalogKind::Module, query).await?;
        let mut items = Vec::with_capacity(entries.items.len());
        for entry in entries.items {
            match entry {
                CatalogEntry::Module(m) => items.push(m),
                _ => anyhow::bail!("list(Module, ..) returned a non-module catalog entry"),
            }
        }
        Ok(Page {
            items,
            next: entries.next,
        })
    }

    /// List stacks matching the given query.
    ///
    /// By default, this routes through `list` and filters stack entries.
    async fn list_stacks(&self, query: &Query) -> anyhow::Result<Page<Stack>> {
        let entries = self.list(CatalogKind::Stack, query).await?;
        let mut items = Vec::with_capacity(entries.items.len());
        for entry in entries.items {
            match entry {
                CatalogEntry::Stack(s) => items.push(s),
                _ => anyhow::bail!("list(Stack, ..) returned a non-stack catalog entry"),
            }
        }
        Ok(Page {
            items,
            next: entries.next,
        })
    }

    //
    // Unified get entrypoint
    //

    /// Unified fetch entrypoint for all catalog kinds.
    async fn get(
        &self,
        kind: CatalogKind,
        name: &str,
        track: &str,
        version: VersionSelector,
    ) -> anyhow::Result<Option<CatalogEntry>>;

    //
    // Providers
    //

    /// Fetch a specific provider version for a given logical provider name and track.
    async fn get_provider(
        &self,
        name: &str,
        track: &str,
        version: VersionSelector,
    ) -> anyhow::Result<Option<Provider>> {
        let entry = self
            .get(CatalogKind::Provider, name, track, version)
            .await?;
        match entry {
            Some(CatalogEntry::Provider(p)) => Ok(Some(p)),
            Some(_) => anyhow::bail!("get(Provider, ..) returned a non-provider catalog entry"),
            None => Ok(None),
        }
    }

    /// Download the binary content for a specific provider.
    async fn download_provider(&self, reference: &CatalogRef) -> anyhow::Result<ContentSource>;

    //
    // Modules
    //

    /// Fetch a specific module version for a given name and track.
    async fn get_module(
        &self,
        name: &str,
        track: &str,
        version: VersionSelector,
    ) -> anyhow::Result<Option<Module>> {
        let entry = self.get(CatalogKind::Module, name, track, version).await?;
        match entry {
            Some(CatalogEntry::Module(m)) => Ok(Some(m)),
            Some(_) => anyhow::bail!("get(Module, ..) returned a non-module catalog entry"),
            None => Ok(None),
        }
    }

    /// Download the binary content for a specific module.
    async fn download_module(&self, reference: &CatalogRef) -> anyhow::Result<ContentSource>;

    //
    // Stacks
    //

    /// Fetch a specific stack version for a given name and track.
    async fn get_stack(
        &self,
        name: &str,
        track: &str,
        version: VersionSelector,
    ) -> anyhow::Result<Option<Stack>> {
        let entry = self.get(CatalogKind::Stack, name, track, version).await?;
        match entry {
            Some(CatalogEntry::Stack(s)) => Ok(Some(s)),
            Some(_) => anyhow::bail!("get(Stack, ..) returned a non-stack catalog entry"),
            None => Ok(None),
        }
    }

    /// Download the binary content for a specific stack version.
    async fn download_stack(&self, reference: &CatalogRef) -> anyhow::Result<ContentSource>;

    //
    // Attachments (e.g. attestations, build info)
    //

    /// List attachment names associated with a catalog entry.
    async fn list_attachments(&self, reference: &CatalogRef) -> anyhow::Result<Vec<String>>;

    /// Download an attachment associated with a catalog entry.
    async fn download_attachment(
        &self,
        reference: &CatalogRef,
        name: &str,
    ) -> anyhow::Result<ContentSource>;
}
