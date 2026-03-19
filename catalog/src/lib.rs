use async_trait::async_trait;
use env_defs::{
    ModuleManifest, ModuleStackData, ModuleVersionDiff, ProviderManifest, ProviderResp,
    StackManifest, TfLockProvider, TfOutput, TfRequiredProvider, TfVariable,
};
use std::path::PathBuf;

/// High-level catalog interface for providers, modules and stacks.
///
/// A catalog entry consists of:
/// - immutable binary content (e.g. zip/tar.gz)
/// - associated metadata that may evolve over time
///
/// The intent is:
/// - **add_*** → create a new version (metadata + bytes)
/// - **promote_*** → evolve metadata to move an entry into a new track/version state
/// - **deprecate_*** → mark a specific entry as deprecated (with a human reason)
/// - **yank_*** → remove an entry from availability for consumers
/// - **download_*** → access the binary content
#[async_trait]
pub trait Catalog: Send + Sync {
    //
    // Providers
    //

    /// Add a new provider (new version) with full data + binary content.
    async fn add_provider(
        &self,
        metadata: &catalog_types::Metadata,
        manifest: &ProviderManifest,
        terraform: &catalog_types::TerraformInterface,
        content: &[u8],
    ) -> anyhow::Result<catalog_types::CatalogRef>;

    /// Promote an existing provider to a new track/version state.
    async fn promote_provider(
        &self,
        reference: &catalog_types::CatalogRef,
        track: String,
        version: Option<String>,
    ) -> anyhow::Result<()>;

    /// Mark an existing provider as deprecated with an explicit reason.
    async fn deprecate_provider(
        &self,
        reference: &catalog_types::CatalogRef,
        reason: String,
    ) -> anyhow::Result<()>;

    /// Yank (disable) an existing provider from availability.
    async fn yank_provider(&self, reference: &catalog_types::CatalogRef) -> anyhow::Result<()>;

    /// Fetch a specific provider version for a given logical provider name and track.
    async fn get_provider(
        &self,
        name: &str,
        track: &str,
        version: catalog_types::VersionSelector,
    ) -> anyhow::Result<Option<catalog_types::Provider>>;

    /// Download the binary content for a specific provider.
    async fn download_provider(
        &self,
        reference: &catalog_types::CatalogRef,
    ) -> anyhow::Result<catalog_types::ContentSource>;

    //
    // Modules
    //

    /// Add a new module (new version) with full data + binary content.
    async fn add_module(
        &self,
        metadata: &catalog_types::Metadata,
        manifest: &ModuleManifest,
        terraform: &catalog_types::TerraformInterface,
        stack_data: Option<ModuleStackData>,
        version_diff: Option<ModuleVersionDiff>,
        content: &[u8],
    ) -> anyhow::Result<catalog_types::CatalogRef>;

    /// Promote an existing module to a new track/version state.
    async fn promote_module(
        &self,
        reference: &catalog_types::CatalogRef,
        track: String,
        version: Option<String>,
    ) -> anyhow::Result<()>;

    /// Mark an existing module as deprecated with an explicit reason.
    async fn deprecate_module(
        &self,
        reference: &catalog_types::CatalogRef,
        reason: String,
    ) -> anyhow::Result<()>;

    /// Yank (disable) an existing module from availability.
    async fn yank_module(&self, reference: &catalog_types::CatalogRef) -> anyhow::Result<()>;

    /// Fetch a specific module version for a given name and track.
    async fn get_module(
        &self,
        name: &str,
        track: &str,
        version: catalog_types::VersionSelector,
    ) -> anyhow::Result<Option<catalog_types::Module>>;

    /// Download the binary content for a specific module.
    async fn download_module(
        &self,
        reference: &catalog_types::CatalogRef,
    ) -> anyhow::Result<catalog_types::ContentSource>;

    //
    // Stacks
    //

    /// Add a new stack (new version) with full data + binary content.
    async fn add_stack(
        &self,
        metadata: &catalog_types::Metadata,
        manifest: &StackManifest,
        terraform: &catalog_types::TerraformInterface,
        version_diff: Option<ModuleVersionDiff>,
        content: &[u8],
    ) -> anyhow::Result<catalog_types::CatalogRef>;

    /// Promote an existing stack to a new track/version state.
    async fn promote_stack(
        &self,
        reference: &catalog_types::CatalogRef,
        track: String,
        version: Option<String>,
    ) -> anyhow::Result<()>;

    /// Mark an existing stack as deprecated with an explicit reason.
    async fn deprecate_stack(
        &self,
        reference: &catalog_types::CatalogRef,
        reason: String,
    ) -> anyhow::Result<()>;

    /// Yank (disable) an existing stack from availability.
    async fn yank_stack(&self, reference: &catalog_types::CatalogRef) -> anyhow::Result<()>;

    /// Fetch a specific stack version for a given name and track.
    async fn get_stack(
        &self,
        name: &str,
        track: &str,
        version: catalog_types::VersionSelector,
    ) -> anyhow::Result<Option<catalog_types::Stack>>;

    /// Download the binary content for a specific stack version.
    async fn download_stack(
        &self,
        reference: &catalog_types::CatalogRef,
    ) -> anyhow::Result<catalog_types::ContentSource>;

    //
    // Listing / queries
    //

    /// Unified listing entrypoint for all catalog kinds.
    ///
    /// Implementors can override this to handle all list queries in one place.
    async fn list(
        &self,
        kind: catalog_types::CatalogKind,
        query: &catalog_types::Query,
    ) -> anyhow::Result<catalog_types::Page<catalog_types::CatalogEntry>>;

    /// List providers matching the given query.
    ///
    /// By default, this routes through `list` and filters provider entries.
    async fn list_providers(
        &self,
        query: &catalog_types::Query,
    ) -> anyhow::Result<catalog_types::Page<catalog_types::Provider>> {
        let entries = self
            .list(catalog_types::CatalogKind::Provider, query)
            .await?;
        let items = entries
            .items
            .into_iter()
            .filter_map(|entry| match entry {
                catalog_types::CatalogEntry::Provider(p) => Some(p),
                _ => None,
            })
            .collect::<Vec<_>>();
        Ok(catalog_types::Page {
            items,
            next: entries.next,
        })
    }

    /// List modules matching the given query.
    ///
    /// By default, this routes through `list` and filters module entries.
    async fn list_modules(
        &self,
        query: &catalog_types::Query,
    ) -> anyhow::Result<catalog_types::Page<catalog_types::Module>> {
        let entries = self.list(catalog_types::CatalogKind::Module, query).await?;
        let items = entries
            .items
            .into_iter()
            .filter_map(|entry| match entry {
                catalog_types::CatalogEntry::Module(m) => Some(m),
                _ => None,
            })
            .collect::<Vec<_>>();
        Ok(catalog_types::Page {
            items,
            next: entries.next,
        })
    }

    /// List stacks matching the given query.
    ///
    /// By default, this routes through `list` and filters stack entries.
    async fn list_stacks(
        &self,
        query: &catalog_types::Query,
    ) -> anyhow::Result<catalog_types::Page<catalog_types::Stack>> {
        let entries = self.list(catalog_types::CatalogKind::Stack, query).await?;
        let items = entries
            .items
            .into_iter()
            .filter_map(|entry| match entry {
                catalog_types::CatalogEntry::Stack(s) => Some(s),
                _ => None,
            })
            .collect::<Vec<_>>();
        Ok(catalog_types::Page {
            items,
            next: entries.next,
        })
    }

    //
    // Attachments (e.g. attestations, build info)
    //

    /// Attach arbitrary binary data (e.g. attestation, build info) to a catalog entry.
    async fn add_attachment(
        &self,
        reference: &catalog_types::CatalogRef,
        name: &str,
        content: &[u8],
    ) -> anyhow::Result<()>;

    /// List attachment names associated with a catalog entry.
    async fn list_attachments(
        &self,
        reference: &catalog_types::CatalogRef,
    ) -> anyhow::Result<Vec<String>>;

    /// Download an attachment associated with a catalog entry.
    async fn download_attachment(
        &self,
        reference: &catalog_types::CatalogRef,
        name: &str,
    ) -> anyhow::Result<catalog_types::ContentSource>;
}

/// Shared types used by the `Catalog` trait.
///
/// For now these are lightweight placeholders so we can discuss
/// the shape of the API. They can later be enriched or replaced
/// with types from `env_defs` or another shared crate.
pub mod catalog_types {
    use super::*;

    /// Bitmask-like selector for which heavy fields should be populated in list responses.
    ///
    /// This crate models projection at the type level only:
    /// - `Query.projection == None` means "Full" (all supported projected fields should be populated).
    /// - `Query.projection == Some(mask)` means "Only populate fields included in `mask`".
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct ProjectionFields {
        bits: u8,
    }

    impl ProjectionFields {
        const METADATA_BIT: u8 = 0b0001;
        const MANIFEST_BIT: u8 = 0b0010;
        const TERRAFORM_BIT: u8 = 0b0100;
        const STACK_DATA_BIT: u8 = 0b1000;

        /// Populate `Provider|Module.metadata`.
        pub const METADATA: ProjectionFields = ProjectionFields {
            bits: Self::METADATA_BIT,
        };
        /// Populate `Provider|Module.manifest`.
        pub const MANIFEST: ProjectionFields = ProjectionFields {
            bits: Self::MANIFEST_BIT,
        };
        /// Populate `Provider|Module|Stack.terraform`.
        pub const TERRAFORM: ProjectionFields = ProjectionFields {
            bits: Self::TERRAFORM_BIT,
        };
        /// Populate `Stack.stack_data`.
        pub const STACK_DATA: ProjectionFields = ProjectionFields {
            bits: Self::STACK_DATA_BIT,
        };

        /// Populate all projected fields.
        pub const ALL: ProjectionFields = ProjectionFields {
            bits: Self::METADATA_BIT
                | Self::MANIFEST_BIT
                | Self::TERRAFORM_BIT
                | Self::STACK_DATA_BIT,
        };

        /// Returns true when this mask includes `flag`.
        pub fn contains(self, flag: ProjectionFields) -> bool {
            (self.bits & flag.bits) != 0
        }
    }

    /// Opaque reference to a stored catalog entry (provider/module/stack version).
    ///
    /// Implementations are free to interpret this as a composite key,
    /// versioned identifier, etc.
    #[derive(Debug, Clone)]
    pub struct CatalogRef {
        pub id: String,
    }

    /// Single metadata struct for providers, modules and stacks.
    #[derive(Debug, Clone, Default)]
    pub struct Metadata {
        pub name: String,
        pub kind: String,
        pub track: String,
        pub version: String,
        pub timestamp: String,
        pub description: String,
        pub reference: String,
        pub cpu: String,
        pub memory: String,
        pub deprecated: bool,
        pub deprecated_message: Option<String>,
    }

    /// Unified Terraform-related interface data used by providers, modules and stacks.
    ///
    /// Some fields may be unused for certain kinds (e.g. providers might not
    /// have outputs); in those cases the corresponding vectors can simply be
    /// left empty.
    #[derive(Debug, Clone, Default)]
    pub struct TerraformInterface {
        pub tf_variables: Vec<TfVariable>,
        pub tf_outputs: Vec<TfOutput>,
        pub tf_providers: Vec<ProviderResp>,
        pub tf_required_providers: Vec<TfRequiredProvider>,
        pub tf_lock_providers: Vec<TfLockProvider>,
        pub tf_extra_environment_variables: Vec<String>,
    }

    /// Where to read or fetch the binary content for catalog entries.
    ///
    /// This is intentionally flexible so that different runtimes (e.g. Lambda
    /// behind API Gateway, long‑running servers, CLIs) can choose the most
    /// appropriate strategy without forcing eager downloads.
    #[derive(Debug, Clone)]
    pub enum ContentSource {
        /// A pre‑signed or otherwise externally reachable URL that the caller can fetch.
        Url(String),
        /// A path on the local filesystem where the content is already available.
        Path(PathBuf),
        /// Raw bytes, for cases where the catalog implementation has already
        /// loaded the content into memory.
        Bytes(Vec<u8>),
    }

    /// Full provider entry as returned from queries.
    #[derive(Debug, Clone)]
    pub struct Provider {
        pub reference: CatalogRef,
        /// When `Query.projection` does not include `metadata`, this should be `None`.
        /// For `Query.projection == None` ("Full"), this should be `Some(...)` when present.
        pub metadata: Option<Metadata>,
        /// When `Query.projection` does not include `manifest`, this should be `None`.
        /// For `Query.projection == None` ("Full"), this should be `Some(...)` when present.
        pub manifest: Option<ProviderManifest>,
        /// When `Query.projection` does not include `terraform`, this should be `None`.
        /// For `Query.projection == None` ("Full"), this should be `Some(...)` when present.
        pub terraform: Option<TerraformInterface>,
    }

    /// Full module entry as returned from queries.
    #[derive(Debug, Clone)]
    pub struct Module {
        pub reference: CatalogRef,
        /// When `Query.projection` does not include `metadata`, this should be `None`.
        /// For `Query.projection == None` ("Full"), this should be `Some(...)` when present.
        pub metadata: Option<Metadata>,
        /// When `Query.projection` does not include `manifest`, this should be `None`.
        /// For `Query.projection == None` ("Full"), this should be `Some(...)` when present.
        pub manifest: Option<ModuleManifest>,
        /// When `Query.projection` does not include `terraform`, this should be `None`.
        /// For `Query.projection == None` ("Full"), this should be `Some(...)` when present.
        pub terraform: Option<TerraformInterface>,
    }

    /// Full stack entry as returned from queries.
    #[derive(Debug, Clone)]
    pub struct Stack {
        pub reference: CatalogRef,
        /// When `Query.projection` does not include `metadata`, this should be `None`.
        /// For `Query.projection == None` ("Full"), this should be `Some(...)` when present.
        pub metadata: Option<Metadata>,
        /// When `Query.projection` does not include `manifest`, this should be `None`.
        /// For `Query.projection == None` ("Full"), this should be `Some(...)` when present.
        pub manifest: Option<StackManifest>,
        /// When `Query.projection` does not include `terraform`, this should be `None`.
        /// For `Query.projection == None` ("Full"), this should be `Some(...)` when present.
        pub terraform: Option<TerraformInterface>,
        /// When `Query.projection` does not include `stack_data`, this should be `None`.
        ///
        /// For `Query.projection == None` ("Full"), this should be `Some(...)` when present.
        pub stack_data: Option<ModuleStackData>,
    }

    /// What kind of catalog entry to operate on.
    #[derive(Debug, Clone, Copy)]
    pub enum CatalogKind {
        Provider,
        Module,
        Stack,
    }

    /// Page-enveloped response returned by all `list*` pagination APIs.
    #[derive(Debug, Clone, Default)]
    pub struct Page<T> {
        pub items: Vec<T>,
        /// Opaque continuation token; present only when the backend truncated results.
        pub next: Option<String>,
    }

    /// Unified query used for all list operations.
    ///
    /// All fields are optional; implementations interpret them as best-effort filters.
    #[derive(Debug, Clone, Default)]
    pub struct Query {
        pub name: Option<String>,
        pub track: Option<String>,
        /// Requested page size (backend may return fewer items).
        pub limit: Option<u32>,
        /// Opaque continuation token provided by the backend (for fetching the next page).
        pub next: Option<String>,
        /// Optional projection mask for which heavy fields should be populated in typed list responses.
        ///
        /// - `None` means "Full" (populate all supported projected fields).
        /// - `Some(mask)` means "Only populate fields included in `mask`".
        pub projection: Option<ProjectionFields>,
    }

    /// Unified representation of a catalog entry when listing generically.
    #[derive(Debug, Clone)]
    pub enum CatalogEntry {
        Provider(Provider),
        Module(Module),
        Stack(Stack),
    }

    /// How to select a version when fetching from the catalog.
    #[derive(Debug, Clone)]
    pub enum VersionSelector {
        /// Use the latest known version for the given name/track.
        Latest,
        /// Use this exact semantic version (or whatever versioning scheme you use).
        Exact(String),
    }
}
