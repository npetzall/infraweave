//! Test-only [`catalog_trait::Catalog`] implementations for unit tests.

use std::sync::{Arc, Mutex};

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

/// Configurable read responses for handler tests (populate/management remain errors).
#[derive(Clone)]
pub struct PresetCatalog {
    inner: Arc<Mutex<PresetInner>>,
}

impl Default for PresetCatalog {
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(PresetInner::default())),
        }
    }
}

#[derive(Clone)]
struct PresetInner {
    list_result: Result<Page<CatalogEntry>, String>,
    get_result: Result<Option<CatalogEntry>, String>,
    download_provider: Result<ContentSource, String>,
    download_module: Result<ContentSource, String>,
    download_stack: Result<ContentSource, String>,
    list_attachments: Result<Vec<String>, String>,
    download_attachment: Result<ContentSource, String>,
    management: Result<(), String>,
}

impl Default for PresetInner {
    fn default() -> Self {
        Self {
            list_result: Ok(Page {
                items: vec![],
                next: None,
            }),
            get_result: Ok(None),
            download_provider: Err("download_provider not configured".into()),
            download_module: Err("download_module not configured".into()),
            download_stack: Err("download_stack not configured".into()),
            list_attachments: Ok(vec![]),
            download_attachment: Err("download_attachment not configured".into()),
            management: Err("management not configured".into()),
        }
    }
}

impl PresetCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_list_page(page: Page<CatalogEntry>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(PresetInner {
                list_result: Ok(page),
                ..Default::default()
            })),
        }
    }

    pub fn with_get(self, r: Result<Option<CatalogEntry>, String>) -> Self {
        self.inner.lock().expect("preset lock").get_result = r;
        self
    }

    pub fn with_download_module(self, r: Result<ContentSource, String>) -> Self {
        self.inner.lock().expect("preset lock").download_module = r;
        self
    }

    pub fn with_list_attachments(self, r: Result<Vec<String>, String>) -> Self {
        self.inner.lock().expect("preset lock").list_attachments = r;
        self
    }

    pub fn with_management_ok(self) -> Self {
        self.inner.lock().expect("preset lock").management = Ok(());
        self
    }
}

#[async_trait]
impl CatalogRead for PresetCatalog {
    async fn list(&self, _kind: CatalogKind, _query: &Query) -> anyhow::Result<Page<CatalogEntry>> {
        let inner = self.inner.lock().expect("preset lock");
        inner.list_result.clone().map_err(|s| anyhow::anyhow!(s))
    }

    async fn get(
        &self,
        _kind: CatalogKind,
        _name: &str,
        _track: &str,
        _version: VersionSelector,
    ) -> anyhow::Result<Option<CatalogEntry>> {
        let inner = self.inner.lock().expect("preset lock");
        inner.get_result.clone().map_err(|s| anyhow::anyhow!(s))
    }

    async fn download_provider(&self, _reference: &CatalogRef) -> anyhow::Result<ContentSource> {
        let inner = self.inner.lock().expect("preset lock");
        inner
            .download_provider
            .clone()
            .map_err(|s| anyhow::anyhow!(s))
    }

    async fn download_module(&self, _reference: &CatalogRef) -> anyhow::Result<ContentSource> {
        let inner = self.inner.lock().expect("preset lock");
        inner
            .download_module
            .clone()
            .map_err(|s| anyhow::anyhow!(s))
    }

    async fn download_stack(&self, _reference: &CatalogRef) -> anyhow::Result<ContentSource> {
        let inner = self.inner.lock().expect("preset lock");
        inner.download_stack.clone().map_err(|s| anyhow::anyhow!(s))
    }

    async fn list_attachments(&self, _reference: &CatalogRef) -> anyhow::Result<Vec<String>> {
        let inner = self.inner.lock().expect("preset lock");
        inner
            .list_attachments
            .clone()
            .map_err(|s| anyhow::anyhow!(s))
    }

    async fn download_attachment(
        &self,
        _reference: &CatalogRef,
        _name: &str,
    ) -> anyhow::Result<ContentSource> {
        let inner = self.inner.lock().expect("preset lock");
        inner
            .download_attachment
            .clone()
            .map_err(|s| anyhow::anyhow!(s))
    }
}

#[async_trait]
impl CatalogPopulate for PresetCatalog {
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
impl CatalogManagement for PresetCatalog {
    async fn promote(
        &self,
        _kind: CatalogKind,
        _reference: &CatalogRef,
        _track: &str,
        _version: Option<&str>,
    ) -> anyhow::Result<()> {
        let inner = self.inner.lock().expect("preset lock");
        inner.management.clone().map_err(|s| anyhow::anyhow!(s))
    }

    async fn deprecate(
        &self,
        _kind: CatalogKind,
        _reference: &CatalogRef,
        _reason: &str,
    ) -> anyhow::Result<()> {
        let inner = self.inner.lock().expect("preset lock");
        inner.management.clone().map_err(|s| anyhow::anyhow!(s))
    }

    async fn yank(&self, _kind: CatalogKind, _reference: &CatalogRef) -> anyhow::Result<()> {
        let inner = self.inner.lock().expect("preset lock");
        inner.management.clone().map_err(|s| anyhow::anyhow!(s))
    }
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
