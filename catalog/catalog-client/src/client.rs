use async_trait::async_trait;
use catalog_trait::read::{CatalogEntry, CatalogKind, ContentSource, Page, Query, VersionSelector};
use catalog_trait::types::{CatalogRef, Metadata, TerraformInterface};
use catalog_trait::{
    Catalog, CatalogManagement, CatalogPopulate, CatalogRead, ModuleManifest, ModuleStackData,
    ProviderManifest, StackManifest,
};

use crate::materialize_content;

/// Wraps a [`Catalog`] and re-exposes it as [`Catalog`], with download entrypoints always returning
/// [`ContentSource::Bytes`] (URLs and paths from the inner catalog are resolved in-process).
///
/// [`Self::new`] accepts **any** `C`. Trait implementations ([`CatalogRead`], [`CatalogPopulate`],
/// [`CatalogManagement`], and thus [`Catalog`]) are only available when `C: Catalog`.
pub struct CatalogClient<C> {
    catalog: C,
}

impl<C> CatalogClient<C> {
    pub fn new(catalog: C) -> Self {
        Self { catalog }
    }
}

async fn to_bytes_source(src: ContentSource) -> anyhow::Result<ContentSource> {
    let bytes = materialize_content(src).await?;
    Ok(ContentSource::Bytes(bytes))
}

#[async_trait]
impl<C: Catalog> CatalogRead for CatalogClient<C> {
    async fn list(&self, kind: CatalogKind, query: &Query) -> anyhow::Result<Page<CatalogEntry>> {
        self.catalog.list(kind, query).await
    }

    async fn get(
        &self,
        kind: CatalogKind,
        name: &str,
        track: &str,
        version: VersionSelector,
    ) -> anyhow::Result<Option<CatalogEntry>> {
        self.catalog.get(kind, name, track, version).await
    }

    async fn download_provider(&self, reference: &CatalogRef) -> anyhow::Result<ContentSource> {
        to_bytes_source(self.catalog.download_provider(reference).await?).await
    }

    async fn download_module(&self, reference: &CatalogRef) -> anyhow::Result<ContentSource> {
        to_bytes_source(self.catalog.download_module(reference).await?).await
    }

    async fn download_stack(&self, reference: &CatalogRef) -> anyhow::Result<ContentSource> {
        to_bytes_source(self.catalog.download_stack(reference).await?).await
    }

    async fn list_attachments(&self, reference: &CatalogRef) -> anyhow::Result<Vec<String>> {
        self.catalog.list_attachments(reference).await
    }

    async fn download_attachment(
        &self,
        reference: &CatalogRef,
        name: &str,
    ) -> anyhow::Result<ContentSource> {
        to_bytes_source(self.catalog.download_attachment(reference, name).await?).await
    }
}

#[async_trait]
impl<C: Catalog> CatalogManagement for CatalogClient<C> {
    async fn promote(
        &self,
        kind: CatalogKind,
        reference: &CatalogRef,
        track: &str,
        version: Option<&str>,
    ) -> anyhow::Result<()> {
        self.catalog.promote(kind, reference, track, version).await
    }

    async fn deprecate(
        &self,
        kind: CatalogKind,
        reference: &CatalogRef,
        reason: &str,
    ) -> anyhow::Result<()> {
        self.catalog.deprecate(kind, reference, reason).await
    }

    async fn yank(&self, kind: CatalogKind, reference: &CatalogRef) -> anyhow::Result<()> {
        self.catalog.yank(kind, reference).await
    }
}

#[async_trait]
impl<C: Catalog> CatalogPopulate for CatalogClient<C> {
    async fn add_provider(
        &self,
        metadata: &Metadata,
        manifest: &ProviderManifest,
        terraform: &TerraformInterface,
        content: &[u8],
    ) -> anyhow::Result<CatalogRef> {
        self.catalog
            .add_provider(metadata, manifest, terraform, content)
            .await
    }

    async fn add_module(
        &self,
        metadata: &Metadata,
        manifest: &ModuleManifest,
        terraform: &TerraformInterface,
        content: &[u8],
    ) -> anyhow::Result<CatalogRef> {
        self.catalog
            .add_module(metadata, manifest, terraform, content)
            .await
    }

    async fn add_stack(
        &self,
        metadata: &Metadata,
        manifest: &StackManifest,
        terraform: &TerraformInterface,
        stack_data: Option<ModuleStackData>,
        content: &[u8],
    ) -> anyhow::Result<CatalogRef> {
        self.catalog
            .add_stack(metadata, manifest, terraform, stack_data, content)
            .await
    }

    async fn add_attachment(
        &self,
        reference: &CatalogRef,
        name: &str,
        content: &[u8],
    ) -> anyhow::Result<()> {
        self.catalog.add_attachment(reference, name, content).await
    }
}
