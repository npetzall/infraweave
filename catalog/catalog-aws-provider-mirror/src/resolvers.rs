//! [`CatalogProviderMirrorResolve`] implementation for the packed S3 mirror layout.

use std::collections::HashMap;
use std::path::PathBuf;

use async_trait::async_trait;
use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::Client as S3Client;
use catalog_trait::read::ContentSource;
use catalog_trait::{CatalogProviderMirrorResolve, TfLockProvider};

use crate::packed;

/// Resolves lockfile providers for one platform to presigned [`ContentSource`] entries for the packed mirror layout.
#[derive(Clone)]
pub struct AwsProviderMirrorResolve {
    s3: S3Client,
    providers_bucket: String,
    presigning: PresigningConfig,
    /// Used when `resolve_provider_mirror` is called with an empty `platform` (after trim).
    default_platform: String,
}

impl AwsProviderMirrorResolve {
    pub fn new(
        s3: S3Client,
        providers_bucket: impl Into<String>,
        presigning: PresigningConfig,
        default_platform: impl Into<String>,
    ) -> Self {
        Self {
            s3,
            providers_bucket: providers_bucket.into(),
            presigning,
            default_platform: default_platform.into(),
        }
    }
}

#[async_trait]
impl CatalogProviderMirrorResolve for AwsProviderMirrorResolve {
    async fn resolve_provider_mirror(
        &self,
        providers: &[TfLockProvider],
        platform: &str,
    ) -> anyhow::Result<HashMap<PathBuf, ContentSource>> {
        let platform = platform.trim();
        let platform = if platform.is_empty() {
            self.default_platform.trim()
        } else {
            platform
        };
        let platforms: Vec<String> = if platform.is_empty() {
            vec![]
        } else {
            vec![platform.to_string()]
        };
        Ok(packed::resolve_packed_provider_mirror(
            &self.s3,
            &self.providers_bucket,
            providers,
            &platforms,
            self.presigning.clone(),
        )
        .await)
    }
}
