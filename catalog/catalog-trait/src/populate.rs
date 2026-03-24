use async_trait::async_trait;
use env_defs::{ModuleManifest, ModuleStackData, ProviderManifest, StackManifest};

use crate::types::{CatalogRef, Metadata, TerraformInterface};

#[async_trait]
pub trait CatalogPopulate: Send + Sync {
    //
    // Providers
    //

    /// Add a new provider (new version) with full data + binary content.
    async fn add_provider(
        &self,
        metadata: &Metadata,
        manifest: &ProviderManifest,
        terraform: &TerraformInterface,
        content: &[u8],
    ) -> anyhow::Result<CatalogRef>;

    //
    // Modules
    //

    /// Add a new module (new version) with full data + binary content.
    async fn add_module(
        &self,
        metadata: &Metadata,
        manifest: &ModuleManifest,
        terraform: &TerraformInterface,
        content: &[u8],
    ) -> anyhow::Result<CatalogRef>;

    //
    // Stacks
    //

    /// Add a new stack (new version) with full data + binary content.
    async fn add_stack(
        &self,
        metadata: &Metadata,
        manifest: &StackManifest,
        terraform: &TerraformInterface,
        stack_data: Option<ModuleStackData>,
        content: &[u8],
    ) -> anyhow::Result<CatalogRef>;

    //
    // Attachments (writes)
    //

    /// Attach arbitrary binary data (e.g. attestation, build info) to a catalog entry.
    async fn add_attachment(
        &self,
        reference: &CatalogRef,
        name: &str,
        content: &[u8],
    ) -> anyhow::Result<()>;
}
