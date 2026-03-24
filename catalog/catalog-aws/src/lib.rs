//! AWS-backed Catalog implementation.
//!
//! See [PLAN.md](../../ai-task/catalog-aws/PLAN.md) for full context.
//! Phase 1 complete: Core scaffolding, AWS clients, config, errors, telemetry.
//! Phase 5: [`compat_models`] and [`compat`] provide internal legacy serde parity (see `ai-task/catalog-aws/PLAN_5.md`).
//! Phase 5b: [`availability`] implements [`CatalogAvailability`](catalog_trait::CatalogAvailability) (single-region; see module docs).

pub mod availability;
mod client;
pub mod compat;
pub mod compat_models;
mod config;
mod errors;
mod management;
mod ops;
pub mod read;
mod telemetry;
mod write;

pub use compat::{catalog_module_to_legacy, catalog_provider_to_legacy, catalog_stack_to_legacy};

pub use client::{default_presign_expiry, AwsClients};
pub use config::Config;
pub use errors::CatalogError;
pub use telemetry::{record_operation, with_telemetry, LatencyBucket, Outcome};

use async_trait::async_trait;
use catalog_trait::read::{CatalogEntry, ContentSource, Page, Query};
use catalog_trait::types::{
    CatalogKind, CatalogRef, Metadata, TerraformInterface, VersionSelector,
};
use catalog_trait::{CatalogManagement, CatalogPopulate, CatalogRead};
use std::sync::Arc;

/// AWS-backed catalog implementation.
///
/// Construct via `AwsCatalog::new(clients)` with dependency-injected AWS clients.
/// Use `AwsCatalog::from_env()` for production (loads config from environment).
#[derive(Clone)]
pub struct AwsCatalog {
    clients: Arc<AwsClients>,
}

impl AwsCatalog {
    /// Create catalog with injected AWS clients.
    pub fn new(clients: AwsClients) -> Self {
        Self {
            clients: Arc::new(clients),
        }
    }

    /// Create catalog from environment (loads config and builds clients).
    pub async fn from_env() -> Result<Self, anyhow::Error> {
        let clients = AwsClients::from_env().await?;
        Ok(Self::new(clients))
    }

    /// Create catalog for unit tests (uses Config::for_test(); clients require local stack).
    pub async fn for_test() -> Result<Self, anyhow::Error> {
        let config = Config::for_test();
        let clients = AwsClients::from_config(config).await?;
        Ok(Self::new(clients))
    }

    /// Reference to underlying clients.
    pub fn clients(&self) -> &AwsClients {
        &self.clients
    }
}

// --- CatalogRead ---

#[async_trait]
impl CatalogRead for AwsCatalog {
    async fn list(&self, kind: CatalogKind, query: &Query) -> anyhow::Result<Page<CatalogEntry>> {
        let start = std::time::Instant::now();
        let result = self.list_impl(kind, query).await;
        telemetry::record_operation(
            "list",
            Some(kind),
            if result.is_ok() {
                telemetry::Outcome::Success
            } else {
                telemetry::Outcome::Failure
            },
            start.elapsed(),
        );
        result
    }

    async fn get(
        &self,
        kind: CatalogKind,
        name: &str,
        track: &str,
        version: VersionSelector,
    ) -> anyhow::Result<Option<CatalogEntry>> {
        let start = std::time::Instant::now();
        let result = self.get_impl(kind, name, track, version).await;
        telemetry::record_operation(
            "get",
            Some(kind),
            if result.is_ok() {
                telemetry::Outcome::Success
            } else {
                telemetry::Outcome::Failure
            },
            start.elapsed(),
        );
        result
    }

    async fn download_provider(&self, reference: &CatalogRef) -> anyhow::Result<ContentSource> {
        self.download_impl(CatalogKind::Provider, reference).await
    }

    async fn download_module(&self, reference: &CatalogRef) -> anyhow::Result<ContentSource> {
        self.download_impl(CatalogKind::Module, reference).await
    }

    async fn download_stack(&self, reference: &CatalogRef) -> anyhow::Result<ContentSource> {
        self.download_impl(CatalogKind::Stack, reference).await
    }

    async fn list_attachments(&self, reference: &CatalogRef) -> anyhow::Result<Vec<String>> {
        self.list_attachments_impl(reference).await
    }

    async fn download_attachment(
        &self,
        reference: &CatalogRef,
        name: &str,
    ) -> anyhow::Result<ContentSource> {
        self.download_attachment_impl(reference, name).await
    }
}

impl AwsCatalog {
    async fn list_impl(
        &self,
        kind: CatalogKind,
        query: &Query,
    ) -> anyhow::Result<Page<CatalogEntry>> {
        let config = self.clients.config();
        let (items, last_key) = ops::execute_list(&self.clients, config, kind, query).await?;

        let projection = query.projection;
        let entries: Vec<CatalogEntry> = match kind {
            CatalogKind::Provider => items
                .iter()
                .filter_map(|item| {
                    read::item_to_provider(item).ok().map(|r| {
                        CatalogEntry::Provider(read::provider_resp_to_catalog(r, projection))
                    })
                })
                .collect(),
            CatalogKind::Module => items
                .iter()
                .filter_map(|item| {
                    read::item_to_module(item)
                        .ok()
                        .map(|r| CatalogEntry::Module(read::module_resp_to_module(&r, projection)))
                })
                .collect(),
            CatalogKind::Stack => items
                .iter()
                .filter_map(|item| {
                    read::item_to_module(item)
                        .ok()
                        .map(|r| CatalogEntry::Stack(read::module_resp_to_stack(&r, projection)))
                })
                .collect(),
        };

        let next = last_key.and_then(|k| read::encode_next_token(&k));

        Ok(Page {
            items: entries,
            next,
        })
    }

    async fn get_impl(
        &self,
        kind: CatalogKind,
        name: &str,
        track: &str,
        version: VersionSelector,
    ) -> anyhow::Result<Option<CatalogEntry>> {
        let config = self.clients.config();
        let item = ops::execute_get(&self.clients, config, kind, name, track, &version).await?;

        let item = match item {
            Some(i) => i,
            None => {
                if matches!(version, VersionSelector::Latest) {
                    return Err(anyhow::Error::new(crate::errors::CatalogError::NotFound {
                        kind: format!("{:?}", kind),
                        key: format!("{}@{} (latest)", name, track),
                        source: None,
                    }));
                }
                return Ok(None);
            }
        };

        let projection = None; // full projection for get
        match kind {
            CatalogKind::Provider => {
                let r = read::item_to_provider(&item)?;
                if r.yanked {
                    return Err(anyhow::Error::new(crate::errors::CatalogError::NotFound {
                        kind: format!("{:?}", kind),
                        key: format!("{} (yanked)", name),
                        source: None,
                    }));
                }
                Ok(Some(CatalogEntry::Provider(
                    read::provider_resp_to_catalog(r, projection),
                )))
            }
            CatalogKind::Module => {
                let r = read::item_to_module(&item)?;
                if r.yanked {
                    return Err(anyhow::Error::new(crate::errors::CatalogError::NotFound {
                        kind: format!("{:?}", kind),
                        key: format!("{}@{} (yanked)", name, track),
                        source: None,
                    }));
                }
                Ok(Some(CatalogEntry::Module(read::module_resp_to_module(
                    &r, projection,
                ))))
            }
            CatalogKind::Stack => {
                let r = read::item_to_module(&item)?;
                if r.yanked {
                    return Err(anyhow::Error::new(crate::errors::CatalogError::NotFound {
                        kind: format!("{:?}", kind),
                        key: format!("{}@{} (yanked)", name, track),
                        source: None,
                    }));
                }
                Ok(Some(CatalogEntry::Stack(read::module_resp_to_stack(
                    &r, projection,
                ))))
            }
        }
    }

    async fn download_impl(
        &self,
        kind: CatalogKind,
        reference: &CatalogRef,
    ) -> anyhow::Result<ContentSource> {
        let s3_key = reference.id.as_str();
        if s3_key.is_empty() {
            anyhow::bail!("CatalogRef has no s3_key");
        }

        let bucket = self.clients.config().bucket_for_kind(kind);
        let presigning_config = self.clients.presigning_config();

        let presigned = self
            .clients
            .s3
            .get_object()
            .bucket(bucket)
            .key(s3_key)
            .presigned(presigning_config)
            .await
            .map_err(|e| anyhow::anyhow!("presign failed: {}", e))?;

        Ok(ContentSource::Url(presigned.uri().to_string()))
    }

    async fn list_attachments_impl(&self, reference: &CatalogRef) -> anyhow::Result<Vec<String>> {
        let s3_key = reference.id.as_str();
        if s3_key.is_empty() {
            return Ok(vec![]);
        }

        let prefix = attachment_prefix(s3_key);
        let bucket = bucket_for_s3_key(self.clients.config(), s3_key);

        let mut list = self
            .clients
            .s3
            .list_objects_v2()
            .bucket(&bucket)
            .prefix(&prefix)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("S3 list failed: {}", e))?;

        let mut names = Vec::new();
        while let Some(contents) = list.contents.take() {
            for obj in contents {
                if let Some(key) = obj.key() {
                    if let Some(name) = key.strip_prefix(&prefix) {
                        if !name.is_empty() && !name.contains('/') {
                            names.push(name.to_string());
                        }
                    }
                }
            }
            if list.is_truncated == Some(true) {
                list = self
                    .clients
                    .s3
                    .list_objects_v2()
                    .bucket(&bucket)
                    .prefix(&prefix)
                    .set_continuation_token(list.next_continuation_token)
                    .send()
                    .await
                    .map_err(|e| anyhow::anyhow!("S3 list failed: {}", e))?;
            } else {
                break;
            }
        }
        names.sort();
        Ok(names)
    }

    async fn download_attachment_impl(
        &self,
        reference: &CatalogRef,
        name: &str,
    ) -> anyhow::Result<ContentSource> {
        let s3_key = reference.id.as_str();
        if s3_key.is_empty() {
            anyhow::bail!("CatalogRef has no s3_key");
        }

        let key = format!("{}{}", attachment_prefix(s3_key), name);
        let bucket = bucket_for_s3_key(self.clients.config(), s3_key);
        let presigning_config = self.clients.presigning_config();

        let presigned = self
            .clients
            .s3
            .get_object()
            .bucket(&bucket)
            .key(&key)
            .presigned(presigning_config)
            .await
            .map_err(|e| anyhow::anyhow!("presign failed: {}", e))?;

        Ok(ContentSource::Url(presigned.uri().to_string()))
    }
}

fn attachment_prefix(s3_key: &str) -> String {
    if let Some(pos) = s3_key.rfind('/') {
        format!("{}/attachments/", &s3_key[..pos])
    } else {
        "attachments/".to_string()
    }
}

fn bucket_for_s3_key(config: &Config, s3_key: &str) -> String {
    if s3_key.starts_with("providers/") {
        config.providers_bucket.clone()
    } else {
        config.modules_bucket.clone()
    }
}

// --- CatalogPopulate ---

#[async_trait]
impl CatalogPopulate for AwsCatalog {
    async fn add_provider(
        &self,
        metadata: &Metadata,
        manifest: &catalog_trait::ProviderManifest,
        terraform: &TerraformInterface,
        content: &[u8],
    ) -> anyhow::Result<CatalogRef> {
        let start = std::time::Instant::now();
        let result = write::execute_add_provider(
            &self.clients,
            self.clients.config(),
            metadata,
            manifest,
            terraform,
            content,
        )
        .await
        .map_err(|e| anyhow::anyhow!("{}", e));
        telemetry::record_operation(
            "add_provider",
            Some(CatalogKind::Provider),
            if result.is_ok() {
                telemetry::Outcome::Success
            } else {
                telemetry::Outcome::Failure
            },
            start.elapsed(),
        );
        result
    }

    async fn add_module(
        &self,
        metadata: &Metadata,
        manifest: &catalog_trait::ModuleManifest,
        terraform: &TerraformInterface,
        content: &[u8],
    ) -> anyhow::Result<CatalogRef> {
        let start = std::time::Instant::now();
        let result = write::execute_add_module(
            &self.clients,
            self.clients.config(),
            metadata,
            manifest,
            terraform,
            content,
        )
        .await
        .map_err(|e| anyhow::anyhow!("{}", e));
        telemetry::record_operation(
            "add_module",
            Some(CatalogKind::Module),
            if result.is_ok() {
                telemetry::Outcome::Success
            } else {
                telemetry::Outcome::Failure
            },
            start.elapsed(),
        );
        result
    }

    async fn add_stack(
        &self,
        metadata: &Metadata,
        manifest: &catalog_trait::StackManifest,
        terraform: &TerraformInterface,
        stack_data: Option<catalog_trait::ModuleStackData>,
        content: &[u8],
    ) -> anyhow::Result<CatalogRef> {
        let start = std::time::Instant::now();
        let result = write::execute_add_stack(
            &self.clients,
            self.clients.config(),
            metadata,
            manifest,
            terraform,
            stack_data,
            content,
        )
        .await
        .map_err(|e| anyhow::anyhow!("{}", e));
        telemetry::record_operation(
            "add_stack",
            Some(CatalogKind::Stack),
            if result.is_ok() {
                telemetry::Outcome::Success
            } else {
                telemetry::Outcome::Failure
            },
            start.elapsed(),
        );
        result
    }

    async fn add_attachment(
        &self,
        reference: &CatalogRef,
        name: &str,
        content: &[u8],
    ) -> anyhow::Result<()> {
        let start = std::time::Instant::now();
        let result = write::execute_add_attachment(
            &self.clients,
            self.clients.config(),
            reference,
            name,
            content,
        )
        .await
        .map_err(|e| anyhow::anyhow!("{}", e));
        telemetry::record_operation(
            "add_attachment",
            None,
            if result.is_ok() {
                telemetry::Outcome::Success
            } else {
                telemetry::Outcome::Failure
            },
            start.elapsed(),
        );
        result
    }
}

// --- CatalogManagement ---

#[async_trait]
impl CatalogManagement for AwsCatalog {
    async fn promote(
        &self,
        kind: CatalogKind,
        reference: &CatalogRef,
        track: &str,
        version: Option<&str>,
    ) -> anyhow::Result<()> {
        let start = std::time::Instant::now();
        let result = management::execute_promote(
            &self.clients,
            self.clients.config(),
            kind,
            reference,
            track,
            version,
        )
        .await
        .map_err(|e| anyhow::anyhow!("{}", e));
        telemetry::record_operation(
            "promote",
            Some(kind),
            if result.is_ok() {
                telemetry::Outcome::Success
            } else {
                telemetry::Outcome::Failure
            },
            start.elapsed(),
        );
        result
    }

    async fn deprecate(
        &self,
        kind: CatalogKind,
        reference: &CatalogRef,
        reason: &str,
    ) -> anyhow::Result<()> {
        let start = std::time::Instant::now();
        let result = management::execute_deprecate(
            &self.clients,
            self.clients.config(),
            kind,
            reference,
            reason,
        )
        .await
        .map_err(|e| anyhow::anyhow!("{}", e));
        telemetry::record_operation(
            "deprecate",
            Some(kind),
            if result.is_ok() {
                telemetry::Outcome::Success
            } else {
                telemetry::Outcome::Failure
            },
            start.elapsed(),
        );
        result
    }

    async fn yank(&self, kind: CatalogKind, reference: &CatalogRef) -> anyhow::Result<()> {
        let start = std::time::Instant::now();
        let result =
            management::execute_yank(&self.clients, self.clients.config(), kind, reference)
                .await
                .map_err(|e| anyhow::anyhow!("{}", e));
        telemetry::record_operation(
            "yank",
            Some(kind),
            if result.is_ok() {
                telemetry::Outcome::Success
            } else {
                telemetry::Outcome::Failure
            },
            start.elapsed(),
        );
        result
    }
}

// Catalog is automatically implemented (CatalogRead + CatalogPopulate + CatalogManagement)
