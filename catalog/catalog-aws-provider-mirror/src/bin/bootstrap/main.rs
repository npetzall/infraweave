//! Lambda `bootstrap`: JSON [`InvokePayload`] in → mirror artifacts to the providers bucket
//! via [`AwsProviderMirrorPopulator::mirror_tf_lock_providers`].
//!
//! Env: `CATALOG_PROVIDER_MIRROR_PLATFORMS` (comma-separated OS/arch, e.g. `linux_amd64`),
//! `CATALOG_PROVIDER_MIRROR_TMPDIR` (staging parent dir, default `/tmp`),
//! `CATALOG_PROVIDER_MIRROR_BUCKET` (Terraform provider mirror bucket; default `providers`),
//! plus usual AWS config (`AWS_REGION`, IAM / instance role for S3).

const CATALOG_PROVIDER_MIRROR_BUCKET_ENV: &str = "CATALOG_PROVIDER_MIRROR_BUCKET";

use aws_sdk_s3::Client as S3Client;
use catalog_aws_provider_mirror::{AwsProviderMirrorPopulator, InvokePayload};
use lambda_runtime::{service_fn, Error, LambdaEvent};
use serde_json::{json, Value};

fn parse_platforms_list(raw: &str) -> Result<Vec<String>, Error> {
    let platforms: Vec<String> = raw
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if platforms.is_empty() {
        return Err(Error::from(
            "platform list is empty; use comma-separated values (e.g. linux_amd64,linux_arm64)",
        ));
    }
    Ok(platforms)
}

fn providers_bucket_from_env() -> String {
    std::env::var(CATALOG_PROVIDER_MIRROR_BUCKET_ENV)
        .unwrap_or_else(|_| "provider_mirror".to_string())
}

fn parse_platforms_from_env() -> Result<Vec<String>, Error> {
    let raw = std::env::var("CATALOG_PROVIDER_MIRROR_PLATFORMS")
        .unwrap_or_else(|_| "linux_amd64,linux_arm64".to_string());
    parse_platforms_list(raw.trim())
}

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .try_init();
}

async fn handle(
    event: LambdaEvent<Value>,
    populator: AwsProviderMirrorPopulator,
) -> Result<Value, Error> {
    let payload: InvokePayload = serde_json::from_value(event.payload.clone())
        .map_err(|e| Error::from(format!("invalid InvokePayload JSON: {e}")))?;

    if payload.providers.is_empty() {
        tracing::warn!("provider_mirror_worker: empty providers list; skipping");
        return Ok(json!({ "ok": true, "skipped": "empty_providers" }));
    }

    let stats = populator
        .mirror_tf_lock_providers(&payload.providers)
        .await
        .map_err(|e| Error::from(format!("mirror_tf_lock_providers: {e:#}")))?;

    tracing::info!(
        correlation_id = ?payload.correlation_id,
        provider_count = payload.providers.len(),
        platform_count = populator.platform_count(),
        ?stats,
        "provider_mirror_worker batch complete"
    );

    Ok(json!({
        "ok": true,
        "artifacts_attempted": stats.artifacts_attempted,
        "artifacts_skipped_existing": stats.artifacts_skipped_existing,
        "artifacts_uploaded": stats.artifacts_uploaded,
        "artifacts_failed": stats.artifacts_failed,
    }))
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    init_tracing();

    let sdk_config = aws_config::load_from_env().await;
    let s3 = S3Client::new(&sdk_config);
    let providers_bucket = providers_bucket_from_env();

    let platforms = parse_platforms_from_env()?;

    let populator = AwsProviderMirrorPopulator::new(s3, providers_bucket, platforms);

    lambda_runtime::run(service_fn(move |event: LambdaEvent<Value>| {
        let p = populator.clone();
        async move { handle(event, p).await }
    }))
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_platforms_splits_and_trims() {
        let p = parse_platforms_list(" linux_amd64 , darwin_arm64 ").expect("parse");
        assert_eq!(p, vec!["linux_amd64", "darwin_arm64"]);
    }

    #[test]
    fn parse_platforms_rejects_empty() {
        assert!(parse_platforms_list("  ,  ").is_err());
    }
}
