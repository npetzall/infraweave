use std::path::PathBuf;
use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use aws_sdk_s3::primitives::ByteStream;
use bytes::Bytes;
use catalog_trait::{CatalogProviderMirrorPopulate, TfLockProvider};
use registry_client::{
    FileArtifact, PlatformDownloadError, ProviderRegistryError, Registry, RegistryClient,
};
use tracing::warn;

use crate::s3_util::head_object_is_not_found;

/// Summary counters after a mirror run (best-effort).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MirrorRunStats {
    pub artifacts_attempted: u64,
    pub artifacts_skipped_existing: u64,
    pub artifacts_uploaded: u64,
    pub artifacts_failed: u64,
}

impl MirrorRunStats {
    pub fn add(&mut self, add: MirrorRunStats) {
        self.artifacts_attempted += add.artifacts_attempted;
        self.artifacts_skipped_existing += add.artifacts_skipped_existing;
        self.artifacts_uploaded += add.artifacts_uploaded;
        self.artifacts_failed += add.artifacts_failed;
    }
}

const MIRROR_STAGING_TMPDIR_ENV: &str = "CATALOG_PROVIDER_MIRROR_TMPDIR";

fn mirror_staging_parent_dir() -> PathBuf {
    std::env::var(MIRROR_STAGING_TMPDIR_ENV)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"))
}

async fn object_exists(s3: &aws_sdk_s3::Client, bucket: &str, key: &str) -> anyhow::Result<bool> {
    match s3.head_object().bucket(bucket).key(key).send().await {
        Ok(_) => Ok(true),
        Err(e) => {
            if head_object_is_not_found(&e) {
                Ok(false)
            } else {
                Err(anyhow::anyhow!("S3 head_object failed for {key}: {e}"))
            }
        }
    }
}

async fn put_object(
    s3: &aws_sdk_s3::Client,
    bucket: &str,
    key: &str,
    body: Bytes,
) -> anyhow::Result<()> {
    s3.put_object()
        .bucket(bucket)
        .key(key)
        .body(ByteStream::from(body))
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("S3 put_object failed for {key}: {e}"))?;
    Ok(())
}

fn parse_platform(target: &str) -> anyhow::Result<(&str, &str)> {
    let parts: Vec<&str> = target.split('_').collect();
    if parts.len() != 2 {
        anyhow::bail!("invalid platform {target:?}: expected os_arch (e.g. linux_amd64)");
    }
    Ok((parts[0], parts[1]))
}

/// Caches one [`RegistryClient`] per registry host (Terraform lock `source` first segment).
pub struct ProviderRegistryProxy {
    pub clients: HashMap<String, RegistryClient>,
}

impl ProviderRegistryProxy {
    pub fn new() -> Self {
        Self {
            clients: HashMap::new(),
        }
    }

    /// Returns a client for the lock provider’s registry host, plus parsed host / namespace / type.
    ///
    /// The host string is the map key: existing clients are cloned; otherwise
    /// [`Registry::new`] and [`RegistryClient::new`] are used, the client is stored, and a clone is returned.
    pub async fn get_for(
        &mut self,
        provider: &TfLockProvider,
    ) -> anyhow::Result<(RegistryClient, String, String, String)> {
        let (host, namespace, typ) = provider.parse_source()?;
        let host = host.to_string();
        let namespace = namespace.to_string();
        let typ = typ.to_string();

        if let Some(c) = self.clients.get(&host) {
            return Ok((c.clone(), host, namespace, typ));
        }

        let reg = Registry::new(host.as_str());
        let client = RegistryClient::new(reg)
            .map_err(|e| anyhow::anyhow!("RegistryClient::new({host:?}): {e}"))?;
        self.clients.insert(host.clone(), client.clone());
        Ok((client, host, namespace, typ))
    }
}

impl Default for ProviderRegistryProxy {
    fn default() -> Self {
        Self::new()
    }
}

fn failed_stats_for_provider(platform_count: usize) -> MirrorRunStats {
    let n = platform_count as u64;
    MirrorRunStats {
        artifacts_attempted: 0,
        artifacts_skipped_existing: 0,
        artifacts_uploaded: 0,
        artifacts_failed: 2 + n,
    }
}

/// AWS provider mirror populate strategy: download from the registry and upload into the provider mirror bucket.
///
/// Implements [`CatalogProviderMirrorPopulate`].
#[derive(Clone)]
pub struct AwsProviderMirrorPopulator {
    s3: aws_sdk_s3::Client,
    providers_bucket: String,
    platforms: Vec<String>,
    registry_proxy: Arc<tokio::sync::Mutex<ProviderRegistryProxy>>,
}

impl AwsProviderMirrorPopulator {
    pub fn new(
        s3: aws_sdk_s3::Client,
        providers_bucket: impl Into<String>,
        platforms: Vec<String>,
    ) -> Self {
        Self {
            s3,
            providers_bucket: providers_bucket.into(),
            platforms,
            registry_proxy: Arc::new(tokio::sync::Mutex::new(ProviderRegistryProxy::new())),
        }
    }

    fn warn_registry(op: &str, source: &str, platform: &str, err: &ProviderRegistryError) {
        warn!("mirror: registry {op} failed source={source} platform={platform}: {err}");
    }

    async fn mirror_one_lock_provider(
        &self,
        registry: RegistryClient,
        provider: TfLockProvider,
        staging_parent: PathBuf,
    ) -> MirrorRunStats {
        let s3 = self.s3.clone();
        let bucket = self.providers_bucket.clone();
        let platforms = self.platforms.clone();

        let mut stats = MirrorRunStats::default();
        let platform_count = platforms.len();

        let (_, namespace, provider_name) = match provider.parse_source() {
            Ok(p) => p,
            Err(e) => {
                warn!("mirror: invalid source {:?}: {}", provider.source, e);
                return failed_stats_for_provider(platform_count);
            }
        };

        for platform in &platforms {
            if let Err(e) = parse_platform(platform) {
                warn!(
                    "mirror: bad platform {:?} source={}: {}",
                    platform, provider.source, e
                );
                return failed_stats_for_provider(platform_count);
            }
        }

        let staging = match tempfile::Builder::new()
            .prefix("catalog-provider-mirror-")
            .tempdir_in(&staging_parent)
        {
            Ok(d) => d,
            Err(e) => {
                warn!(
                    "mirror: tempdir under {:?} failed source={}: {e}",
                    staging_parent, provider.source
                );
                return failed_stats_for_provider(platform_count);
            }
        };
        let dir = staging.path();

        let prov = match registry.provider().await {
            Ok(p) => p,
            Err(e) => {
                Self::warn_registry("provider", &provider.source, "", &e);
                return failed_stats_for_provider(platform_count);
            }
        };

        let report = match prov
            .download(
                namespace,
                provider_name,
                &provider.version,
                &platforms,
                dir.to_path_buf(),
            )
            .await
        {
            Ok(r) => r,
            Err(e) => {
                Self::warn_registry("download", &provider.source, "", &e);
                return failed_stats_for_provider(platform_count);
            }
        };

        let mut entries: Vec<FileArtifact> = Vec::new();
        for platform in &platforms {
            match report.get(platform.as_str()) {
                None => {
                    warn!(
                        "mirror: missing registry download entry for platform {:?} source={}",
                        platform, provider.source
                    );
                }
                Some(Ok(files)) => entries.extend(files.iter().cloned()),
                Some(Err(e)) => match e {
                    PlatformDownloadError::Download(err) => {
                        Self::warn_registry("download", &provider.source, platform, err)
                    }
                    PlatformDownloadError::Validate(err) => {
                        Self::warn_registry("validate", &provider.source, platform, err)
                    }
                },
            }
        }

        if entries.is_empty() {
            return stats;
        }

        let mut uploads: Vec<(String, Bytes)> = Vec::with_capacity(entries.len());
        for FileArtifact { filename, path } in entries {
            let body = match tokio::fs::read(&path).await {
                Ok(b) => b,
                Err(e) => {
                    warn!(
                        "mirror: read staged file {:?} source={}: {e}",
                        path, provider.source
                    );
                    return failed_stats_for_provider(platform_count);
                }
            };
            uploads.push((format!("{}/{}", provider.source, filename), body.into()));
        }

        for (key, body) in uploads {
            stats.artifacts_attempted += 1;
            match object_exists(&s3, &bucket, &key).await {
                Ok(true) => {
                    stats.artifacts_skipped_existing += 1;
                    continue;
                }
                Ok(false) => {}
                Err(e) => {
                    warn!("mirror: head failed key={key}: {e}");
                    stats.artifacts_failed += 1;
                    continue;
                }
            }

            if let Err(e) = put_object(&s3, &bucket, &key, body).await {
                warn!("mirror: put failed key={key}: {e}");
                stats.artifacts_failed += 1;
                continue;
            }

            stats.artifacts_uploaded += 1;
        }

        stats
    }

    /// Mirror registry provider zips, `SHA256SUMS`, and detached signatures for each provider × platform.
    ///
    /// Providers are mirrored concurrently. Each provider’s artifacts are staged on disk under the temp
    /// directory, verified, then uploaded. Layout matches [`catalog_aws::mirror::mirror_tf_lock_providers`].
    async fn mirror_tf_lock_providers_impl(
        &self,
        providers: &[TfLockProvider],
    ) -> anyhow::Result<MirrorRunStats> {
        let mut total = MirrorRunStats::default();
        if providers.is_empty() {
            return Ok(total);
        }

        let staging_parent = mirror_staging_parent_dir();

        let mut join_set = tokio::task::JoinSet::new();
        for provider in providers {
            let pop = self.clone();
            let provider = provider.clone();
            let staging_parent = staging_parent.clone();
            let (registry, _, _, _) = self.registry_proxy.lock().await.get_for(&provider).await?;
            join_set.spawn(async move {
                pop.mirror_one_lock_provider(registry.clone(), provider, staging_parent)
                    .await
            });
        }

        while let Some(joined) = join_set.join_next().await {
            match joined {
                Ok(part) => total.add(part),
                Err(e) => warn!("mirror: provider task join error: {e}"),
            }
        }

        Ok(total)
    }

    /// Mirror provider artifacts into S3 using [`RegistryClient`] (OpenTofu registry protocol).
    ///
    /// Each lockfile provider is processed concurrently. Downloads are staged in a subdirectory of
    /// **`CATALOG_PROVIDER_MIRROR_TMPDIR`** (default **`/tmp`** if unset or blank), then SHA256 and
    /// detached GPG signatures are verified before upload.
    pub async fn mirror_tf_lock_providers(
        &self,
        providers: &[TfLockProvider],
    ) -> anyhow::Result<MirrorRunStats> {
        self.mirror_tf_lock_providers_impl(providers).await
    }

    pub fn platform_count(&self) -> usize {
        self.platforms.len()
    }
}

#[async_trait]
impl CatalogProviderMirrorPopulate for AwsProviderMirrorPopulator {
    async fn ensure_providers_mirrored(&self, providers: &[TfLockProvider]) -> anyhow::Result<()> {
        self.mirror_tf_lock_providers(providers).await?;
        Ok(())
    }
}
