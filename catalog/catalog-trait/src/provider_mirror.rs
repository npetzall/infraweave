use async_trait::async_trait;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::read::ContentSource;
use crate::TfLockProvider;

/// Populate / warm a provider filesystem mirror from registry (or equivalent) sources.
///
/// Which platforms and artifacts are mirrored is defined by the implementation (e.g. configuration),
/// not by this trait’s parameters.
#[async_trait]
pub trait CatalogProviderMirrorPopulate: Send + Sync {
    /// Ensure provider artifacts for the given lockfile entries exist in the mirror.
    async fn ensure_providers_mirrored(&self, providers: &[TfLockProvider]) -> anyhow::Result<()>;
}

/// Resolve mirror-relative paths to content sources for Terraform provider mirrors.
#[async_trait]
pub trait CatalogProviderMirrorResolve: Send + Sync {
    /// Map relative mirror paths to [`ContentSource`] (e.g. presigned URLs).
    ///
    /// Implementations may return a partial map; omissions are expected, matching
    /// [`crate::read::Module::provider_mirror`] / [`crate::read::Stack::provider_mirror`] semantics.
    ///
    /// If `platform` is empty after trimming, implementations use their configured default platform
    /// (if any).
    async fn resolve_provider_mirror(
        &self,
        providers: &[TfLockProvider],
        platform: &str,
    ) -> anyhow::Result<HashMap<PathBuf, ContentSource>>;
}
