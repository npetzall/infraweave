//! Configuration loading with environment variables and test/local overrides.
//!
//! Environment-aware behavior matches `env_aws_direct` assumptions for local test parity.

use std::time::Duration;

/// Default presign expiry when not overridden.
pub const DEFAULT_PRESIGN_EXPIRY_SECS: u64 = 3600;

/// Default DynamoDB table names for catalog kinds (local development).
pub const DEFAULT_MODULES_TABLE: &str = "modules";
pub const DEFAULT_PROVIDERS_TABLE: &str = "providers";
pub const DEFAULT_STACKS_TABLE: &str = "stacks";

/// Default S3 bucket names (local development).
pub const DEFAULT_MODULES_BUCKET: &str = "modules";
pub const DEFAULT_PROVIDERS_BUCKET: &str = "providers";
pub const DEFAULT_STACKS_BUCKET: &str = "stacks";

/// Runtime configuration for AwsCatalog.
#[derive(Debug, Clone)]
pub struct Config {
    /// AWS region.
    pub region: String,
    /// Whether running in local/test mode (custom endpoints).
    pub local_mode: bool,
    /// DynamoDB endpoint override (when local_mode).
    pub dynamodb_endpoint: Option<String>,
    /// S3 endpoint override (when local_mode).
    pub s3_endpoint: Option<String>,
    /// Presign URL expiry.
    pub presign_expiry: Duration,
    /// DynamoDB table name for modules.
    pub modules_table: String,
    /// DynamoDB table name for providers.
    pub providers_table: String,
    /// DynamoDB table name for stacks.
    pub stacks_table: String,
    /// S3 bucket for modules.
    pub modules_bucket: String,
    /// S3 bucket for providers.
    pub providers_bucket: String,
    /// S3 bucket for stacks.
    pub stacks_bucket: String,
}

impl Config {
    /// Load configuration from environment variables.
    ///
    /// Local/test overrides:
    /// - `TEST_MODE` or `DYNAMODB_ENDPOINT` set → local_mode, use custom DynamoDB endpoint
    /// - `AWS_ENDPOINT_URL_S3` or `MINIO_ENDPOINT` → custom S3 endpoint
    /// - `CATALOG_PRESIGN_EXPIRY_SECS` → override default presign expiry
    pub fn from_env() -> Result<Self, anyhow::Error> {
        let region = std::env::var("AWS_REGION")
            .map_err(|_| anyhow::anyhow!("AWS_REGION environment variable must be set"))?;

        let local_mode =
            std::env::var("TEST_MODE").is_ok() || std::env::var("DYNAMODB_ENDPOINT").is_ok();

        let dynamodb_endpoint = std::env::var("DYNAMODB_ENDPOINT")
            .or_else(|_| std::env::var("AWS_ENDPOINT_URL_DYNAMODB"))
            .ok();

        let s3_endpoint = std::env::var("AWS_ENDPOINT_URL_S3")
            .or_else(|_| std::env::var("MINIO_ENDPOINT"))
            .ok();

        let presign_expiry_secs = std::env::var("CATALOG_PRESIGN_EXPIRY_SECS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(DEFAULT_PRESIGN_EXPIRY_SECS);
        let presign_expiry = Duration::from_secs(presign_expiry_secs);

        let modules_table = std::env::var("DYNAMODB_MODULES_TABLE_NAME")
            .unwrap_or_else(|_| DEFAULT_MODULES_TABLE.to_string());
        let providers_table = std::env::var("DYNAMODB_PROVIDERS_TABLE_NAME")
            .unwrap_or_else(|_| DEFAULT_PROVIDERS_TABLE.to_string());
        let stacks_table = std::env::var("DYNAMODB_STACKS_TABLE_NAME")
            .unwrap_or_else(|_| DEFAULT_STACKS_TABLE.to_string());

        let modules_bucket = std::env::var("MODULE_S3_BUCKET")
            .unwrap_or_else(|_| DEFAULT_MODULES_BUCKET.to_string());
        let providers_bucket = std::env::var("PROVIDERS_S3_BUCKET")
            .unwrap_or_else(|_| DEFAULT_PROVIDERS_BUCKET.to_string());
        let stacks_bucket =
            std::env::var("STACKS_S3_BUCKET").unwrap_or_else(|_| DEFAULT_STACKS_BUCKET.to_string());

        Ok(Self {
            region,
            local_mode,
            dynamodb_endpoint,
            s3_endpoint,
            presign_expiry,
            modules_table,
            providers_table,
            stacks_table,
            modules_bucket,
            providers_bucket,
            stacks_bucket,
        })
    }

    /// Create config for unit tests (no real AWS calls).
    ///
    /// Uses dummy values; clients should be mocked or not used.
    pub fn for_test() -> Self {
        Self {
            region: "us-west-2".to_string(),
            local_mode: true,
            dynamodb_endpoint: Some("http://localhost:8000".to_string()),
            s3_endpoint: Some("http://localhost:9000".to_string()),
            presign_expiry: Duration::from_secs(DEFAULT_PRESIGN_EXPIRY_SECS),
            modules_table: DEFAULT_MODULES_TABLE.to_string(),
            providers_table: DEFAULT_PROVIDERS_TABLE.to_string(),
            stacks_table: DEFAULT_STACKS_TABLE.to_string(),
            modules_bucket: DEFAULT_MODULES_BUCKET.to_string(),
            providers_bucket: DEFAULT_PROVIDERS_BUCKET.to_string(),
            stacks_bucket: DEFAULT_STACKS_BUCKET.to_string(),
        }
    }

    /// Table name for the given catalog kind.
    pub fn table_for_kind(&self, kind: catalog_trait::types::CatalogKind) -> &str {
        match kind {
            catalog_trait::types::CatalogKind::Provider => &self.providers_table,
            catalog_trait::types::CatalogKind::Module => &self.modules_table,
            catalog_trait::types::CatalogKind::Stack => &self.stacks_table,
        }
    }

    /// Bucket name for the given catalog kind.
    pub fn bucket_for_kind(&self, kind: catalog_trait::types::CatalogKind) -> &str {
        match kind {
            catalog_trait::types::CatalogKind::Provider => &self.providers_bucket,
            catalog_trait::types::CatalogKind::Module => &self.modules_bucket,
            catalog_trait::types::CatalogKind::Stack => &self.stacks_bucket,
        }
    }
}
