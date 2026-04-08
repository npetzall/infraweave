//! Configuration loading with environment variables and test/local overrides.
//!
//! Environment-aware behavior matches `env_aws_direct` assumptions for local test parity.

use std::time::Duration;

/// Default registry API hostname for mirror key layout and read-side projection (align with `REGISTRY_API_HOSTNAME` on the mirror worker).
pub const DEFAULT_REGISTRY_API_HOSTNAME: &str = "registry.opentofu.org";

/// Default presign expiry when not overridden.
pub const DEFAULT_PRESIGN_EXPIRY_SECS: u64 = 3600;

/// Default DynamoDB table names for catalog kinds (local development).
pub const DEFAULT_MODULES_TABLE: &str = "modules";
pub const DEFAULT_PROVIDERS_TABLE: &str = "providers";
pub const DEFAULT_STACKS_TABLE: &str = "stacks";

/// Default S3 bucket names (local development).
pub const DEFAULT_MODULES_BUCKET: &str = "modules";
pub const DEFAULT_PROVIDERS_BUCKET: &str = "providers";
pub const DEFAULT_PROVIDER_MIRROR_BUCKET: &str = "provider_mirror";
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
    /// S3 bucket for mirrored Terraform registry artifacts (`provider_mirror` read projection).
    /// Defaults to [`Self::providers_bucket`] when `CATALOG_PROVIDER_MIRROR_BUCKET` is unset.
    pub provider_mirror_bucket: String,
    /// S3 bucket for stacks.
    pub stacks_bucket: String,
    /// Worker Lambda ARN for async provider mirror (`lambda:InvokeFunction` `Event`). If unset, mirror is skipped.
    pub provider_mirror_arn: Option<String>,
    /// When false, never invoke mirror even if [`Self::provider_mirror_arn`] is set.
    /// Unset env in local/test mode defaults to disabled; in non-local defaults to enabled.
    pub provider_mirror_enabled: bool,
    /// Optional Lambda endpoint (e.g. LocalStack) when [`Self::local_mode`] is true.
    pub lambda_endpoint: Option<String>,
    /// Registry API host segment for mirrored provider S3 keys and for registry HTTPS calls during `provider_mirror` projection.
    /// Must match the mirror worker `REGISTRY_API_HOSTNAME` env (see IaC docs).
    pub registry_api_hostname: String,
    /// Platforms for mirror upload / Lambda worker (comma-separated env on worker: `CATALOG_PROVIDER_MIRROR_PLATFORMS`).
    pub provider_mirror_platforms: Vec<String>,
}

impl Config {
    /// Load configuration from environment variables.
    ///
    /// Local/test overrides:
    /// - `TEST_MODE` or `DYNAMODB_ENDPOINT` set → local_mode, use custom DynamoDB endpoint
    /// - `AWS_ENDPOINT_URL_S3` or `MINIO_ENDPOINT` → custom S3 endpoint
    /// - `CATALOG_PRESIGN_EXPIRY_SECS` → override default presign expiry
    /// - `CATALOG_PROVIDER_MIRROR_ARN` → async mirror worker; optional `CATALOG_PROVIDER_MIRROR_ENABLED`
    /// - `REGISTRY_API_HOSTNAME` → registry host for mirror keys + `provider_mirror` projection (default OpenTofu registry)
    /// - `CATALOG_PROVIDER_MIRROR_PLATFORMS` → comma-separated platforms for projection (default `linux_amd64,linux_arm64`)
    /// - `CATALOG_PROVIDER_MIRROR_BUCKET` → S3 bucket for `provider_mirror` keys (default: same as `PROVIDERS_S3_BUCKET`)
    /// - `AWS_ENDPOINT_URL_LAMBDA` → optional Lambda endpoint in local mode
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
        let provider_mirror_bucket = std::env::var("CATALOG_PROVIDER_MIRROR_BUCKET")
            .unwrap_or_else(|_| DEFAULT_PROVIDER_MIRROR_BUCKET.to_string());
        let stacks_bucket =
            std::env::var("STACKS_S3_BUCKET").unwrap_or_else(|_| DEFAULT_STACKS_BUCKET.to_string());

        let provider_mirror_arn = std::env::var("CATALOG_PROVIDER_MIRROR_ARN")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let provider_mirror_enabled = match std::env::var("CATALOG_PROVIDER_MIRROR_ENABLED") {
            Ok(s) if s.eq_ignore_ascii_case("false") || s == "0" => false,
            Ok(s) if s.eq_ignore_ascii_case("true") || s == "1" => true,
            Ok(_) => true,
            Err(_) => !local_mode,
        };

        let lambda_endpoint = std::env::var("AWS_ENDPOINT_URL_LAMBDA").ok();

        let registry_api_hostname = std::env::var("REGISTRY_API_HOSTNAME")
            .unwrap_or_else(|_| DEFAULT_REGISTRY_API_HOSTNAME.to_string())
            .trim()
            .to_string();

        let provider_mirror_platforms = provider_mirror_platforms_from_env();

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
            provider_mirror_bucket,
            stacks_bucket,
            provider_mirror_arn,
            provider_mirror_enabled,
            lambda_endpoint,
            registry_api_hostname,
            provider_mirror_platforms,
        })
    }

    /// Whether publish path should attempt async mirror invokes (ARN present, non-empty, and enabled).
    #[must_use]
    pub fn provider_mirror_should_invoke(&self) -> bool {
        self.provider_mirror_enabled
            && self
                .provider_mirror_arn
                .as_ref()
                .is_some_and(|a| !a.trim().is_empty())
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
            provider_mirror_bucket: DEFAULT_PROVIDER_MIRROR_BUCKET.to_string(),
            stacks_bucket: DEFAULT_STACKS_BUCKET.to_string(),
            provider_mirror_arn: None,
            provider_mirror_enabled: false,
            lambda_endpoint: None,
            registry_api_hostname: DEFAULT_REGISTRY_API_HOSTNAME.to_string(),
            provider_mirror_platforms: vec!["linux_amd64".to_string(), "linux_arm64".to_string()],
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

fn provider_mirror_platforms_from_env() -> Vec<String> {
    let raw = std::env::var("CATALOG_PROVIDER_MIRROR_PLATFORMS")
        .unwrap_or_else(|_| "linux_amd64,linux_arm64".to_string());
    let parsed: Vec<String> = raw
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if parsed.is_empty() {
        vec!["linux_amd64".to_string(), "linux_arm64".to_string()]
    } else {
        parsed
    }
}
