//! AWS client construction (DynamoDB, S3) with presign configuration.
//!
//! Environment-aware behavior for local test parity (same assumptions as `env_aws_direct`).

use aws_sdk_dynamodb::config::BehaviorVersion;
use aws_sdk_s3::presigning::PresigningConfig;
use std::time::Duration;

use crate::config::{Config, DEFAULT_PRESIGN_EXPIRY_SECS};

/// AWS clients bundle for catalog operations.
#[derive(Clone)]
pub struct AwsClients {
    pub dynamodb: aws_sdk_dynamodb::Client,
    pub s3: aws_sdk_s3::Client,
    config: Config,
}

impl AwsClients {
    /// Build clients from environment-derived configuration.
    pub async fn from_env() -> Result<Self, anyhow::Error> {
        let config = Config::from_env()?;
        Self::from_config(config).await
    }

    /// Build clients from explicit configuration.
    pub async fn from_config(config: Config) -> Result<Self, anyhow::Error> {
        let region = aws_config::Region::new(config.region.clone());

        let dynamodb = if config.local_mode {
            let endpoint = config.dynamodb_endpoint.as_ref().ok_or_else(|| {
                anyhow::anyhow!(
                    "DYNAMODB_ENDPOINT or AWS_ENDPOINT_URL_DYNAMODB must be set in local mode"
                )
            })?;
            log::info!("Local mode: Using DynamoDB endpoint: {}", endpoint);

            let credentials = aws_sdk_dynamodb::config::Credentials::new(
                std::env::var("AWS_ACCESS_KEY_ID").unwrap_or_else(|_| "minio".to_string()),
                std::env::var("AWS_SECRET_ACCESS_KEY").unwrap_or_else(|_| "minio123".to_string()),
                None,
                None,
                "local",
            );

            let cfg = aws_sdk_dynamodb::Config::builder()
                .behavior_version(BehaviorVersion::latest())
                .credentials_provider(credentials)
                .region(region.clone())
                .endpoint_url(endpoint)
                .build();

            aws_sdk_dynamodb::Client::from_conf(cfg)
        } else {
            let mut loader = aws_config::from_env();
            loader = loader.region(region.clone());
            let sdk_config = loader.load().await;
            aws_sdk_dynamodb::Client::new(&sdk_config)
        };

        let s3 = if config.local_mode {
            let endpoint = config.s3_endpoint.as_ref().ok_or_else(|| {
                anyhow::anyhow!("AWS_ENDPOINT_URL_S3 or MINIO_ENDPOINT must be set in local mode")
            })?;
            log::info!("Local mode: Using S3 endpoint: {}", endpoint);

            let credentials = aws_sdk_s3::config::Credentials::new(
                std::env::var("AWS_ACCESS_KEY_ID").unwrap_or_else(|_| "minio".to_string()),
                std::env::var("AWS_SECRET_ACCESS_KEY").unwrap_or_else(|_| "minio123".to_string()),
                None,
                None,
                "local",
            );

            let cfg = aws_sdk_s3::Config::builder()
                .behavior_version(aws_sdk_s3::config::BehaviorVersion::latest())
                .credentials_provider(credentials)
                .region(region)
                .force_path_style(true)
                .endpoint_url(endpoint)
                .build();

            aws_sdk_s3::Client::from_conf(cfg)
        } else {
            let mut loader = aws_config::from_env();
            loader = loader.region(region);
            let sdk_config = loader.load().await;
            aws_sdk_s3::Client::new(&sdk_config)
        };

        Ok(Self {
            dynamodb,
            s3,
            config,
        })
    }

    /// Presigning config with explicit expiry (default from config or 3600s).
    pub fn presigning_config(&self) -> PresigningConfig {
        PresigningConfig::expires_in(self.config.presign_expiry).expect(
            "presign expiry must be between 1s and 604800s (7 days); \
             CATALOG_PRESIGN_EXPIRY_SECS should be in that range",
        )
    }

    /// Presigning config with custom expiry.
    pub fn presigning_config_with_expiry(&self, expiry: Duration) -> PresigningConfig {
        PresigningConfig::expires_in(expiry).expect("presign expiry must be between 1s and 604800s")
    }

    /// Reference to underlying config.
    pub fn config(&self) -> &Config {
        &self.config
    }
}

/// Default presign expiry for presigner setup.
pub fn default_presign_expiry() -> Duration {
    Duration::from_secs(DEFAULT_PRESIGN_EXPIRY_SECS)
}
