//! [`Registry`] facade: service discovery and resolution of `providers.v1` / `modules.v1`.

use std::sync::Arc;
use std::time::Duration;

use reqwest::header::HeaderMap;
use reqwest::Url;
use tokio::sync::Mutex;

use crate::error::ProviderRegistryError;
use crate::http_util::fetch_json;
use crate::module_registry::ModuleRegistry;
use crate::provider_registry::ProviderRegistry;

/// Public Terraform and OpenTofu registries: fixed `/v1/providers` and `/v1/modules` (no discovery).
const BUILTIN_REGISTRY_HOSTS: &[&str] = &["registry.terraform.io", "registry.opentofu.org"];

/// Registry identity: hostname (and optional port), e.g. `registry.terraform.io` or `127.0.0.1:9`.
///
/// Builtin hosts [`registry.terraform.io`](https://registry.terraform.io) and
/// [`registry.opentofu.org`](https://registry.opentofu.org) skip `/.well-known/terraform.json` and use
/// `https://{host}/v1/providers` and `https://{host}/v1/modules` directly.
///
/// Use [`Self::provider`] / [`Self::module`] for resolved API bases and URL builders.
#[derive(Debug, Clone)]
pub struct Registry {
    host: String,
    request_headers: Option<HeaderMap>,
    discovery_cache: Arc<Mutex<Option<CachedDiscovery>>>,
}

#[derive(Debug, Clone)]
struct CachedDiscovery {
    doc: TerraformServiceDiscovery,
    final_url: Url,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct TerraformServiceDiscovery {
    #[serde(rename = "providers.v1")]
    providers_v1: Option<String>,
    #[serde(rename = "modules.v1")]
    modules_v1: Option<String>,
}

impl Registry {
    /// Stores `host` verbatim (e.g. `registry.terraform.io` or `127.0.0.1:9`).
    pub fn new(host: impl Into<String>) -> Self {
        Self {
            host: host.into(),
            request_headers: None,
            discovery_cache: Arc::new(Mutex::new(None)),
        }
    }

    /// Merges these headers into discovery and (via [`crate::RegistryClient`]) registry API requests.
    pub fn with_request_headers(mut self, headers: HeaderMap) -> Self {
        self.request_headers = Some(headers);
        self.discovery_cache = Arc::new(Mutex::new(None));
        self
    }

    pub(crate) fn request_headers(&self) -> Option<&HeaderMap> {
        self.request_headers.as_ref()
    }

    fn trimmed_host(&self) -> Result<&str, ProviderRegistryError> {
        let host = self.host.trim();
        if host.is_empty() {
            return Err(ProviderRegistryError::EmptyRegistryHost);
        }
        Ok(host)
    }

    /// Providers API root and REST URL helpers.
    pub async fn provider(
        &self,
        http: &reqwest::Client,
    ) -> Result<ProviderRegistry, ProviderRegistryError> {
        let host = self.trimmed_host()?;
        let base_str = if Self::is_builtin_host(host) {
            Self::conventional_providers_base_url(host)
        } else {
            let cached = self.fetch_service_discovery(http, host).await?;
            let raw = cached
                .doc
                .providers_v1
                .as_ref()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .ok_or_else(|| ProviderRegistryError::MissingProvidersV1 {
                    url: cached.final_url.to_string(),
                })?;
            Self::resolve_service_v1_base(raw, &cached.final_url)?
        };
        let base = Url::parse(base_str.trim()).map_err(ProviderRegistryError::InvalidBaseUrl)?;
        Ok(ProviderRegistry::from_base(base))
    }

    /// Modules API root and REST URL helpers.
    pub async fn module(
        &self,
        http: &reqwest::Client,
    ) -> Result<ModuleRegistry, ProviderRegistryError> {
        let host = self.trimmed_host()?;
        let base_str = if Self::is_builtin_host(host) {
            Self::conventional_modules_base_url(host)
        } else {
            let cached = self.fetch_service_discovery(http, host).await?;
            let raw = cached
                .doc
                .modules_v1
                .as_ref()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .ok_or_else(|| ProviderRegistryError::MissingModulesV1 {
                    url: cached.final_url.to_string(),
                })?;
            Self::resolve_service_v1_base(raw, &cached.final_url)?
        };
        let base = Url::parse(base_str.trim()).map_err(ProviderRegistryError::InvalidBaseUrl)?;
        Ok(ModuleRegistry::from_base(base))
    }

    pub(crate) fn is_builtin_host(host: &str) -> bool {
        let h = host.trim();
        BUILTIN_REGISTRY_HOSTS
            .iter()
            .any(|known| known.eq_ignore_ascii_case(h))
    }

    fn conventional_providers_base_url(host: &str) -> String {
        format!("https://{}/v1/providers", host.trim_end_matches('/'))
    }

    fn conventional_modules_base_url(host: &str) -> String {
        format!("https://{}/v1/modules", host.trim_end_matches('/'))
    }

    pub(crate) fn service_discovery_uses_http(host: &str) -> bool {
        let h = host.trim();
        h.starts_with("127.0.0.1:")
            || h.starts_with("[::1]:")
            || h.starts_with("localhost:")
            || h == "localhost"
    }

    fn service_discovery_url(registry_host: &str) -> String {
        let host = registry_host.trim();
        let scheme = if Self::service_discovery_uses_http(host) {
            "http"
        } else {
            "https"
        };
        format!("{scheme}://{host}/.well-known/terraform.json")
    }

    async fn fetch_service_discovery(
        &self,
        http: &reqwest::Client,
        registry_host: &str,
    ) -> Result<CachedDiscovery, ProviderRegistryError> {
        if let Some(cached) = self.discovery_cache.lock().await.clone() {
            return Ok(cached);
        }

        let discovery_url = Self::service_discovery_url(registry_host);
        let (body, discovery_final) = fetch_json(
            http,
            &discovery_url,
            Duration::from_secs(10),
            self.request_headers(),
        )
        .await?;
        let doc: TerraformServiceDiscovery = serde_json::from_str(&body)
            .map_err(|e| ProviderRegistryError::json(discovery_final.as_str(), e))?;
        let cached = CachedDiscovery {
            doc,
            final_url: discovery_final,
        };
        *self.discovery_cache.lock().await = Some(cached.clone());
        Ok(cached)
    }

    /// Resolves a `providers.v1` / `modules.v1` entry relative to the final discovery URL (after redirects).
    pub(crate) fn resolve_service_v1_base(
        raw: &str,
        discovery_url: &Url,
    ) -> Result<String, ProviderRegistryError> {
        let raw = raw.trim();
        if raw.starts_with("http://") || raw.starts_with("https://") {
            return Ok(raw.to_string());
        }
        discovery_url
            .join(raw)
            .map_err(|source| ProviderRegistryError::UrlResolve {
                base: discovery_url.to_string(),
                reference: raw.to_string(),
                source,
            })
            .map(|u| u.to_string())
    }
}

#[cfg(test)]
#[path = "registry_test.rs"]
mod tests;
