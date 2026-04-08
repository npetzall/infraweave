use async_trait::async_trait;
use catalog_trait::{CatalogProviderMirrorPopulate, TfLockProvider};

/// [`CatalogProviderMirrorPopulate`] that does nothing and returns success immediately.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoopProviderMirrorPopulate;

#[async_trait]
impl CatalogProviderMirrorPopulate for NoopProviderMirrorPopulate {
    async fn ensure_providers_mirrored(&self, _providers: &[TfLockProvider]) -> anyhow::Result<()> {
        Ok(())
    }
}
