//! Packed filesystem-mirror layout: `{source}/terraform-provider-{TYPE}_{VERSION}_{TARGET}.zip`.
//!
//! `source` is the full lockfile string (`host/namespace/type`, e.g. `registry.opentofu.org/kreuzwerker/docker`).
//! The mirror worker must store zips under the same keys for resolution to succeed.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use aws_sdk_s3::presigning::PresigningConfig;
use catalog_trait::read::ContentSource;
use catalog_trait::TfLockProvider;

use crate::s3_util::head_object_is_not_found;

/// S3 object key for the distribution zip in packed layout.
#[must_use]
fn packed_distribution_zip_key(
    provider: &TfLockProvider,
    platform: &str,
) -> anyhow::Result<String> {
    let source = provider.source.trim();
    let version = provider.version.trim();
    let (_, _, ptype) = provider.parse_source()?;
    let platform = platform.trim();
    Ok(format!(
        "{}/terraform-provider-{}_{}_{}.zip",
        source, ptype, version, platform
    ))
}

/// Map mirror paths to presigned GET URLs; skips missing keys and failures (best-effort, like registry-based projection).
pub(crate) async fn resolve_packed_provider_mirror(
    s3: &aws_sdk_s3::Client,
    providers_bucket: &str,
    providers: &[TfLockProvider],
    platforms: &[String],
    presigning: PresigningConfig,
) -> HashMap<PathBuf, ContentSource> {
    let mut out = HashMap::new();
    if platforms.is_empty() {
        return out;
    }

    let mut seen: HashSet<(&str, &str)> = HashSet::new();
    for provider in providers {
        if !seen.insert((provider.source.as_str(), provider.version.as_str())) {
            continue;
        }

        for platform in platforms {
            let key = match packed_distribution_zip_key(provider, platform) {
                Ok(k) => k,
                Err(e) => {
                    log::debug!(
                        "provider_mirror packed: skip source={:?}: {}",
                        provider.source,
                        e
                    );
                    continue;
                }
            };

            match s3
                .head_object()
                .bucket(providers_bucket)
                .key(&key)
                .send()
                .await
            {
                Ok(_) => {}
                Err(e) => {
                    if head_object_is_not_found(&e) {
                        log::debug!("provider_mirror packed: missing key={key}");
                    } else {
                        log::debug!("provider_mirror packed: head failed key={key}: {e}");
                    }
                    continue;
                }
            }

            let url = match s3
                .get_object()
                .bucket(providers_bucket)
                .key(&key)
                .presigned(presigning.clone())
                .await
            {
                Ok(p) => p.uri().to_string(),
                Err(e) => {
                    log::debug!("provider_mirror packed: presign failed key={key}: {e}");
                    continue;
                }
            };

            out.insert(PathBuf::from(&key), ContentSource::Url(url));
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packed_zip_key_matches_three_segment_source() {
        let k = packed_distribution_zip_key(
            &TfLockProvider {
                source: "registry.opentofu.org/hashicorp/aws".to_string(),
                version: "5.0.0".to_string(),
            },
            "linux_amd64",
        )
        .expect("key");
        assert_eq!(
            k,
            "registry.opentofu.org/hashicorp/aws/terraform-provider-aws_5.0.0_linux_amd64.zip"
        );
    }

    #[test]
    fn packed_zip_key_rejects_short_source() {
        assert!(packed_distribution_zip_key(
            &TfLockProvider {
                source: "a/b".to_string(),
                version: "1".to_string(),
            },
            "linux_amd64",
        )
        .is_err());
    }
}
