//! Error types for registry HTTP and JSON handling.

use std::fmt;

use reqwest::StatusCode;

/// What comparison failed for [`ProviderRegistryError::ShasumMismatch`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShasumMismatchDetail {
    /// Registry package `shasum` did not match the entry for this file in the signed `SHA256SUMS` manifest.
    RegistryVsShasumsFile,
    /// Zip digest did not match the entry in the signed `SHA256SUMS` manifest.
    VsShasumsFile,
}

impl fmt::Display for ShasumMismatchDetail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RegistryVsShasumsFile => {
                write!(
                    f,
                    "registry package metadata shasum does not match signed SHA256SUMS manifest"
                )
            }
            Self::VsShasumsFile => write!(f, "does not match signed SHA256SUMS manifest"),
        }
    }
}

/// Errors returned by [`crate::Registry`], [`crate::RegistryClient`], and [`crate::ProviderRegistryClient`] operations.
#[derive(Debug, thiserror::Error)]
pub enum ProviderRegistryError {
    #[error("HTTP error for {url}: {source}")]
    Http {
        url: String,
        #[source]
        source: reqwest::Error,
    },

    #[error("registry returned {status} for {url}")]
    UnsuccessfulStatus { url: String, status: StatusCode },

    #[error("JSON error for {url}: {source}")]
    Json {
        url: String,
        #[source]
        source: serde_json::Error,
    },

    #[error("unexpected Content-Type for {url}: {content_type:?} (expected application/json)")]
    UnexpectedContentType {
        url: String,
        content_type: Option<String>,
    },

    #[error("package metadata at {url} does not match requested platform: expected {expected_os}/{expected_arch}, got {got_os}/{got_arch}")]
    PlatformMetadataMismatch {
        url: String,
        expected_os: String,
        expected_arch: String,
        got_os: String,
        got_arch: String,
    },

    #[error("artifact URL must use HTTPS for non-loopback hosts: {url}")]
    InsecureArtifactUrl { url: String },

    #[error("invalid base URL: {0}")]
    InvalidBaseUrl(#[from] url::ParseError),

    #[error("empty registry hostname")]
    EmptyRegistryHost,

    #[error("invalid platform {platform:?}: expected os_arch (e.g. linux_amd64)")]
    InvalidPlatform { platform: String },

    #[error("service discovery document at {url} did not define providers.v1")]
    MissingProvidersV1 { url: String },

    #[error("service discovery document at {url} did not define modules.v1")]
    MissingModulesV1 { url: String },

    #[error(
        "module download at {url} did not return a source location (X-Terraform-Get or location)"
    )]
    MissingModuleSourceLocation { url: String },

    #[error("unsupported module source location scheme (only http/https supported): {location}")]
    UnsupportedModuleSourceScheme { location: String },

    #[error("could not resolve relative URL {reference} against {base}: {source}")]
    UrlResolve {
        base: String,
        reference: String,
        #[source]
        source: url::ParseError,
    },

    #[error("SHA256SUMS is not valid UTF-8")]
    InvalidShasumsUtf8,

    #[error("no GPG public keys in registry metadata; cannot verify SHA256SUMS signature")]
    NoGpgPublicKeys,

    #[error("failed to parse detached GPG signature: {source}")]
    ParseDetachedSignature {
        #[source]
        source: pgp::errors::Error,
    },

    #[error("failed to parse GPG public key {key_id}: {source}")]
    ParseGpgPublicKey {
        key_id: String,
        #[source]
        source: pgp::errors::Error,
    },

    #[error("GPG signature verification failed for SHA256SUMS")]
    GpgSignatureVerificationFailed,

    #[error("no SHA256SUMS line for {filename:?}")]
    ShasumNotInManifest { filename: String },

    #[error("SHA256 mismatch for {filename}: {detail}")]
    ShasumMismatch {
        filename: String,
        detail: ShasumMismatchDetail,
    },

    #[error("I/O error writing provider artifacts: {0}")]
    Io(#[from] std::io::Error),
}

/// Per-platform failure after HTTP download or after local verification ([`crate::ProviderPackage::validate`]).
#[derive(Debug, thiserror::Error)]
pub enum PlatformDownloadError {
    #[error("download failed: {0}")]
    Download(#[source] ProviderRegistryError),
    #[error("validation failed: {0}")]
    Validate(#[source] ProviderRegistryError),
}

impl ProviderRegistryError {
    pub(crate) fn http(url: impl Into<String>, source: reqwest::Error) -> Self {
        Self::Http {
            url: url.into(),
            source,
        }
    }

    pub(crate) fn json(url: impl Into<String>, source: serde_json::Error) -> Self {
        Self::Json {
            url: url.into(),
            source,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{PlatformDownloadError, ProviderRegistryError, ShasumMismatchDetail};

    #[test]
    fn shasum_mismatch_detail_display() {
        assert_eq!(
            ShasumMismatchDetail::RegistryVsShasumsFile.to_string(),
            "registry package metadata shasum does not match signed SHA256SUMS manifest"
        );
        assert_eq!(
            ShasumMismatchDetail::VsShasumsFile.to_string(),
            "does not match signed SHA256SUMS manifest"
        );
    }

    #[test]
    fn platform_download_error_wraps_registry_error() {
        let inner = ProviderRegistryError::EmptyRegistryHost;
        let err = PlatformDownloadError::Download(inner);
        assert!(err.to_string().contains("download failed"));
    }

    #[test]
    fn unexpected_content_type_display_includes_url() {
        let err = ProviderRegistryError::UnexpectedContentType {
            url: "http://127.0.0.1:1/.well-known/terraform.json".into(),
            content_type: Some("text/plain".into()),
        };
        let msg = err.to_string();
        assert!(msg.contains("text/plain"));
        assert!(msg.contains("terraform.json"));
    }

    #[test]
    fn platform_metadata_mismatch_display() {
        let err = ProviderRegistryError::PlatformMetadataMismatch {
            url: "http://x/meta".into(),
            expected_os: "linux".into(),
            expected_arch: "amd64".into(),
            got_os: "darwin".into(),
            got_arch: "arm64".into(),
        };
        assert!(err.to_string().contains("linux/amd64"));
        assert!(err.to_string().contains("darwin/arm64"));
    }
}
