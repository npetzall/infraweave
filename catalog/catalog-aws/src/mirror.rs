//! Terraform/OpenTofu registry provider mirror into the catalog **provider mirror** S3 bucket
//! ([`Config::provider_mirror_bucket`](crate::config::Config::provider_mirror_bucket)).
//!
//! S3 key layout and registry download/upload live in the internal `mirror_tf_lock` module (used by
//! read-side `provider_mirror` projection).
//!
//! `build_aws_provider_mirror` wires populate (Lambda vs no-op) and S3 resolve once at
//! [`AwsClients::from_config`](crate::client::AwsClients::from_config).
//!
//! [`AwsProviderMirror`] implements [`catalog_trait::CatalogProviderMirrorPopulate`] and
//! [`catalog_trait::CatalogProviderMirrorResolve`] for the AWS S3 provider mirror (resolve via
//! [`catalog_aws_provider_mirror::AwsProviderMirrorResolve`]). Use
//! [`AwsClients::provider_mirror`](crate::client::AwsClients::provider_mirror).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use aws_sdk_lambda::Client as LambdaClient;
use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::Client as S3Client;
use catalog_aws_provider_mirror::{
    AwsProviderMirrorResolve, LambdaProviderMirrorPopulate, NoopProviderMirrorPopulate,
};
use catalog_trait::read::ContentSource;
use catalog_trait::{CatalogProviderMirrorPopulate, CatalogProviderMirrorResolve, TfLockProvider};

use crate::config::Config;

/// Provider mirror resolve plus a [`CatalogProviderMirrorPopulate`] implementation (Lambda or no-op).
#[derive(Clone)]
pub struct AwsProviderMirror {
    populate: Arc<dyn CatalogProviderMirrorPopulate>,
    resolve: AwsProviderMirrorResolve,
}

impl AwsProviderMirror {
    pub(crate) fn new(
        populate: Arc<dyn CatalogProviderMirrorPopulate>,
        resolve: AwsProviderMirrorResolve,
    ) -> Self {
        Self { populate, resolve }
    }
}

#[async_trait]
impl CatalogProviderMirrorPopulate for AwsProviderMirror {
    async fn ensure_providers_mirrored(&self, providers: &[TfLockProvider]) -> anyhow::Result<()> {
        self.populate.ensure_providers_mirrored(providers).await
    }
}

#[async_trait]
impl CatalogProviderMirrorResolve for AwsProviderMirror {
    async fn resolve_provider_mirror(
        &self,
        providers: &[TfLockProvider],
        platform: &str,
    ) -> anyhow::Result<HashMap<PathBuf, ContentSource>> {
        self.resolve
            .resolve_provider_mirror(providers, platform)
            .await
    }
}

/// Constructed once in [`AwsClients::from_config`](crate::client::AwsClients::from_config).
#[must_use]
pub(crate) fn build_aws_provider_mirror(
    lambda: &Option<LambdaClient>,
    s3: &S3Client,
    config: &Config,
    presigning_config: PresigningConfig,
) -> AwsProviderMirror {
    AwsProviderMirror::new(
        aws_provider_mirror_populate(lambda, config),
        aws_provider_mirror_resolve(s3, config, presigning_config),
    )
}

fn aws_provider_mirror_populate(
    lambda: &Option<LambdaClient>,
    config: &Config,
) -> Arc<dyn CatalogProviderMirrorPopulate> {
    if config.provider_mirror_should_invoke() {
        if let Some(lambda) = lambda.as_ref() {
            if let Some(arn) = config
                .provider_mirror_arn
                .as_ref()
                .filter(|a| !a.trim().is_empty())
            {
                return Arc::new(LambdaProviderMirrorPopulate::new(
                    lambda.clone(),
                    arn.clone(),
                ));
            }
        }
    }
    Arc::new(NoopProviderMirrorPopulate)
}

fn aws_provider_mirror_resolve(
    s3: &S3Client,
    config: &Config,
    presigning_config: PresigningConfig,
) -> AwsProviderMirrorResolve {
    let default_platform = config
        .provider_mirror_platforms
        .first()
        .cloned()
        .unwrap_or_default();
    AwsProviderMirrorResolve::new(
        s3.clone(),
        config.provider_mirror_bucket.clone(),
        presigning_config,
        default_platform,
    )
}
