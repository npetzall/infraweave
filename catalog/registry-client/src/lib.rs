//! Client for the [OpenTofu provider registry protocol](https://opentofu.org/docs/internals/provider-registry-protocol/).
//!
//! Build a [`Registry`] with [`Registry::new`] (registry hostname or `host:port`), then either call async [`Registry::provider`]
//! or build [`RegistryClient`] and [`RegistryClient::provider`], then use [`ProviderRegistryClient::download`]
//! to fetch provider zips, `SHA256SUMS`, detached signatures, and a combined GPG keyring per platform
//! ([`DownloadReport`] maps each platform id to artifacts or a per-platform error).

mod client;
mod error;
mod keyring;
mod provider_package;
mod registry;

pub use client::{DownloadReport, PlatformArtifactResult, ProviderRegistryClient, RegistryClient};
pub use error::{PlatformDownloadError, ProviderRegistryError, ShasumMismatchDetail};
pub use provider_package::{FileArtifact, ProviderPackage};
pub use registry::{ProviderRegistry, Registry};
