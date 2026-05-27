//! Facade HTTP client bound to a registry host ([`Registry`]).

use std::time::Duration;

use reqwest::header::HeaderMap;

use crate::error::ProviderRegistryError;
use crate::registry::Registry;

fn default_http_client() -> Result<reqwest::Client, ProviderRegistryError> {
    const DEFAULT_USER_AGENT: &str =
        concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));
    reqwest::Client::builder()
        .user_agent(DEFAULT_USER_AGENT)
        .connect_timeout(Duration::from_secs(5))
        .read_timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .map_err(|e| ProviderRegistryError::http("<client build>", e))
}

pub use crate::module_client::{ModulePackage, ModuleRegistryClient};
pub use crate::provider_client::{DownloadReport, PlatformArtifactResult, ProviderRegistryClient};

/// HTTP client for provider and module registry APIs on one host.
#[derive(Debug, Clone)]
pub struct RegistryClient {
    http: reqwest::Client,
    registry: Registry,
}

impl RegistryClient {
    pub fn new(registry: Registry) -> Result<Self, ProviderRegistryError> {
        let http = default_http_client()?;
        Ok(Self { http, registry })
    }

    pub fn with_http_client(registry: Registry, http: reqwest::Client) -> Self {
        Self { http, registry }
    }

    /// Attaches headers to discovery and registry API requests (e.g. `Authorization`).
    pub fn with_request_headers(self, headers: HeaderMap) -> Self {
        Self {
            http: self.http,
            registry: self.registry.with_request_headers(headers),
        }
    }

    /// Resolved providers API ([`ProviderRegistry`](crate::ProviderRegistry)) and shared HTTP client.
    pub async fn provider(&self) -> Result<ProviderRegistryClient, ProviderRegistryError> {
        let provider = self.registry.provider(&self.http).await?;
        Ok(ProviderRegistryClient::new(
            provider,
            self.http.clone(),
            self.registry.request_headers().cloned(),
        ))
    }

    /// Resolved modules API ([`ModuleRegistry`](crate::ModuleRegistry)) and shared HTTP client.
    pub async fn module(&self) -> Result<ModuleRegistryClient, ProviderRegistryError> {
        let module = self.registry.module(&self.http).await?;
        Ok(ModuleRegistryClient::new(
            module,
            self.http.clone(),
            self.registry.request_headers().cloned(),
        ))
    }
}
