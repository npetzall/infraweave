//! Download **known** provider and module versions from OpenTofu/Terraform registries.
//!
//! - [Provider registry protocol](https://opentofu.org/docs/internals/provider-registry-protocol/)
//! - [Module registry protocol](https://opentofu.org/docs/internals/module-registry-protocol/)
//!
//! ## Scope
//!
//! - Service discovery (`/.well-known/terraform.json`) or builtin public hosts
//! - Providers: `GET …/{namespace}/{type}/{version}/download/{os}/{arch}` plus artifact download and GPG/SHA256 verification
//! - Modules: `GET …/{namespace}/{name}/{system}/{version}/download` → source location → `http(s)` archive download
//!
//! Not implemented: version listing (`…/versions`), `login.v1`, dependency-lock `packages` metadata,
//! non-HTTP(S) module sources (`git::`, etc.), or automatic credentials-helper / env auth
//! (use [`Registry::with_request_headers`]).
//!
//! ## Intentional deviations
//!
//! - Loopback registries use **HTTP** for discovery (tests and local mocks).
//! - Artifact `http://` URLs are allowed only when the artifact host is loopback; otherwise HTTPS is required.
//! - Builtin [`registry.terraform.io`](https://registry.terraform.io) and [`registry.opentofu.org`](https://registry.opentofu.org)
//!   skip discovery and use `https://{host}/v1/providers` and `https://{host}/v1/modules`.
//! - Module archives are not checksum- or GPG-verified by this crate.
//!
//! ## Usage
//!
//! ```ignore
//! let client = RegistryClient::new(Registry::new("registry.opentofu.org"))?;
//! let providers = client.provider().await?;
//! let modules = client.module().await?;
//! ```

mod client;
mod error;
mod http_util;
mod keyring;
mod module_client;
mod module_registry;
mod provider_client;
mod provider_package;
mod provider_registry;
mod registry;

pub use client::{
    DownloadReport, ModulePackage, ModuleRegistryClient, PlatformArtifactResult,
    ProviderRegistryClient, RegistryClient,
};
pub use error::{PlatformDownloadError, ProviderRegistryError, ShasumMismatchDetail};
pub use module_registry::ModuleRegistry;
pub use provider_package::{FileArtifact, ProviderPackage};
pub use provider_registry::ProviderRegistry;
pub use registry::Registry;
