//! Test-only [`catalog_trait::Catalog`] stub for integration tests (`--no-default-features` uses
//! [`StubCatalog`]; handler coverage lives in `catalog-http`).

use async_trait::async_trait;
use catalog_trait::read::{CatalogEntry, ContentSource, Page, Query};
use catalog_trait::types::{
    CatalogKind, CatalogRef, Metadata, TerraformInterface, VersionSelector,
};
use catalog_trait::{
    CatalogManagement, CatalogPopulate, CatalogRead, ModuleManifest, ModuleStackData,
    ProviderManifest, StackManifest,
};

fn stub_err() -> anyhow::Error {
    anyhow::anyhow!("StubCatalog: not implemented (tests should not call this)")
}

#[derive(Clone, Copy, Debug, Default)]
pub struct StubCatalog;

#[async_trait]
impl CatalogRead for StubCatalog {
    async fn list(&self, _kind: CatalogKind, _query: &Query) -> anyhow::Result<Page<CatalogEntry>> {
        Ok(Page {
            items: vec![],
            next: None,
        })
    }

    async fn get(
        &self,
        _kind: CatalogKind,
        _name: &str,
        _track: &str,
        _version: VersionSelector,
    ) -> anyhow::Result<Option<CatalogEntry>> {
        Ok(None)
    }

    async fn download_provider(&self, _reference: &CatalogRef) -> anyhow::Result<ContentSource> {
        Err(stub_err())
    }

    async fn download_module(&self, _reference: &CatalogRef) -> anyhow::Result<ContentSource> {
        Err(stub_err())
    }

    async fn download_stack(&self, _reference: &CatalogRef) -> anyhow::Result<ContentSource> {
        Err(stub_err())
    }

    async fn list_attachments(&self, _reference: &CatalogRef) -> anyhow::Result<Vec<String>> {
        Ok(vec![])
    }

    async fn download_attachment(
        &self,
        _reference: &CatalogRef,
        _name: &str,
    ) -> anyhow::Result<ContentSource> {
        Err(stub_err())
    }
}

#[async_trait]
impl CatalogPopulate for StubCatalog {
    async fn add_provider(
        &self,
        _metadata: &Metadata,
        _manifest: &ProviderManifest,
        _terraform: &TerraformInterface,
        _content: &[u8],
    ) -> anyhow::Result<CatalogRef> {
        Err(stub_err())
    }

    async fn add_module(
        &self,
        _metadata: &Metadata,
        _manifest: &ModuleManifest,
        _terraform: &TerraformInterface,
        _content: &[u8],
    ) -> anyhow::Result<CatalogRef> {
        Err(stub_err())
    }

    async fn add_stack(
        &self,
        _metadata: &Metadata,
        _manifest: &StackManifest,
        _terraform: &TerraformInterface,
        _stack_data: Option<ModuleStackData>,
        _content: &[u8],
    ) -> anyhow::Result<CatalogRef> {
        Err(stub_err())
    }

    async fn add_attachment(
        &self,
        _reference: &CatalogRef,
        _name: &str,
        _content: &[u8],
    ) -> anyhow::Result<()> {
        Err(stub_err())
    }
}

#[async_trait]
impl CatalogManagement for StubCatalog {
    async fn promote(
        &self,
        _kind: CatalogKind,
        _reference: &CatalogRef,
        _track: &str,
        _version: Option<&str>,
    ) -> anyhow::Result<()> {
        Err(stub_err())
    }

    async fn deprecate(
        &self,
        _kind: CatalogKind,
        _reference: &CatalogRef,
        _reason: &str,
    ) -> anyhow::Result<()> {
        Err(stub_err())
    }

    async fn yank(&self, _kind: CatalogKind, _reference: &CatalogRef) -> anyhow::Result<()> {
        Err(stub_err())
    }
}
