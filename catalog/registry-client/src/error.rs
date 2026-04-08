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

    #[error("invalid base URL: {0}")]
    InvalidBaseUrl(#[from] url::ParseError),

    #[error("empty registry hostname")]
    EmptyRegistryHost,

    #[error("invalid platform {platform:?}: expected os_arch (e.g. linux_amd64)")]
    InvalidPlatform { platform: String },

    #[error("service discovery document at {url} did not define providers.v1")]
    MissingProvidersV1 { url: String },

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
