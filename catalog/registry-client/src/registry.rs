//! Terraform `.well-known` service discovery for `providers.v1`,
//! [`Registry`] for the registry host, and [`ProviderRegistry`] for normalized `/v1/providers`
//! API URLs and JSON response types.

use std::time::Duration;

use reqwest::Url;

use crate::error::ProviderRegistryError;

/// Public Terraform and OpenTofu registries: fixed `/v1/providers` layout (no discovery request).
const BUILTIN_REGISTRY_HOSTS: &[&str] = &["registry.terraform.io", "registry.opentofu.org"];

/// Registry identity: hostname (and optional port), e.g. `registry.terraform.io` or `127.0.0.1:9`.
/// Use [`Self::provider`] for the resolved providers API base and URL builders.
#[derive(Debug, Clone)]
pub struct Registry {
    host: String,
}

impl Registry {
    /// Stores `host` verbatim (e.g. `registry.terraform.io` or `127.0.0.1:9`).
    pub fn new(host: impl Into<String>) -> Self {
        Self { host: host.into() }
    }

    /// Providers API root and REST URL helpers.
    ///
    /// Public registries → `https://{host}/v1/providers`; otherwise `GET https://{host}/.well-known/terraform.json`
    /// and use `providers.v1`.
    pub async fn provider(
        &self,
        http: &reqwest::Client,
    ) -> Result<ProviderRegistry, ProviderRegistryError> {
        let host = self.host.trim();
        if host.is_empty() {
            return Err(ProviderRegistryError::EmptyRegistryHost);
        }
        let base_str = if Self::is_builtin_host(host) {
            Self::conventional_providers_base_url(host)
        } else {
            Self::providers_v1_base_from_http(http, host).await?
        };
        let base = Url::parse(base_str.trim()).map_err(ProviderRegistryError::InvalidBaseUrl)?;
        Ok(ProviderRegistry { base })
    }

    fn is_builtin_host(host: &str) -> bool {
        let h = host.trim();
        BUILTIN_REGISTRY_HOSTS
            .iter()
            .any(|known| known.eq_ignore_ascii_case(h))
    }

    fn conventional_providers_base_url(host: &str) -> String {
        format!("https://{}/v1/providers", host.trim_end_matches('/'))
    }

    fn service_discovery_uses_http(host: &str) -> bool {
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

    async fn providers_v1_base_from_http(
        http: &reqwest::Client,
        registry_host: &str,
    ) -> Result<String, ProviderRegistryError> {
        let discovery_url = Self::service_discovery_url(registry_host);
        let response = http
            .get(&discovery_url)
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| ProviderRegistryError::http(discovery_url.clone(), e))?;
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| ProviderRegistryError::http(discovery_url.clone(), e))?;
        if !status.is_success() {
            return Err(ProviderRegistryError::UnsuccessfulStatus {
                url: discovery_url.clone(),
                status,
            });
        }
        let doc: TerraformServiceDiscovery = serde_json::from_str(&body)
            .map_err(|e| ProviderRegistryError::json(&discovery_url, e))?;
        let raw = doc
            .providers_v1
            .as_ref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| ProviderRegistryError::MissingProvidersV1 {
                url: discovery_url.clone(),
            })?;
        Self::resolve_providers_v1_base(raw, registry_host)
    }

    fn resolve_providers_v1_base(
        raw: &str,
        registry_host: &str,
    ) -> Result<String, ProviderRegistryError> {
        if raw.starts_with("http://") || raw.starts_with("https://") {
            return Ok(raw.to_string());
        }
        let origin = if Self::service_discovery_uses_http(registry_host) {
            format!("http://{}/", registry_host.trim_end_matches('/'))
        } else {
            format!("https://{}/", registry_host.trim_end_matches('/'))
        };
        let base = Url::parse(&origin).map_err(ProviderRegistryError::InvalidBaseUrl)?;
        base.join(raw)
            .map_err(|source| ProviderRegistryError::UrlResolve {
                base: origin,
                reference: raw.to_string(),
                source,
            })
            .map(|u| u.to_string())
    }
}

/// Provider registry REST API base (`…/v1/providers` or discovered equivalent).
#[derive(Debug, Clone)]
pub struct ProviderRegistry {
    base: Url,
}

impl ProviderRegistry {
    /// Resolved providers API base (discovery may include a trailing `/` on the path).
    pub fn base_url(&self) -> &Url {
        &self.base
    }

    /// `GET …/{namespace}/{type}/{version}/download/{os}/{arch}`
    pub fn provider_package_url(
        &self,
        namespace: &str,
        provider_type: &str,
        version: &str,
        os: &str,
        arch: &str,
    ) -> Url {
        let mut u = self.base.clone();
        u.path_segments_mut()
            .expect("provider registry base URL must support path segments")
            .pop_if_empty()
            .extend([namespace, provider_type, version, "download", os, arch]);
        u
    }
}

// --- JSON types for provider registry REST responses (see OpenTofu provider registry protocol) ---

/// Response from `GET .../:namespace/:type/:version/download/:os/:arch`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct PackageDownload {
    pub protocols: Vec<String>,
    pub os: String,
    pub arch: String,
    pub filename: String,
    pub download_url: String,
    pub shasums_url: String,
    pub shasums_signature_url: String,
    pub shasum: String,
    pub signing_keys: SigningKeys,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct SigningKeys {
    pub gpg_public_keys: Vec<GpgPublicKey>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct GpgPublicKey {
    pub key_id: String,
    pub ascii_armor: String,
    #[serde(default)]
    pub trust_signature: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub source_url: String,
}

#[derive(serde::Deserialize)]
struct TerraformServiceDiscovery {
    #[serde(rename = "providers.v1")]
    providers_v1: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_registry_hosts_case_insensitive() {
        assert!(Registry::is_builtin_host("REGISTRY.TERRAFORM.IO"));
        assert!(Registry::is_builtin_host("Registry.OpenTofu.Org"));
        assert!(!Registry::is_builtin_host("registry.example.com"));
    }

    #[test]
    fn resolve_providers_v1_relative_to_loopback() {
        assert_eq!(
            Registry::resolve_providers_v1_base("/api/v1/providers/", "127.0.0.1:9").unwrap(),
            "http://127.0.0.1:9/api/v1/providers/"
        );
    }

    #[tokio::test]
    async fn provider_registry_urls() {
        let reg = Registry::new("registry.terraform.io");
        let http = reqwest::Client::new();
        let pr = reg.provider(&http).await.expect("provider");
        assert_eq!(
            pr.provider_package_url("hashicorp", "aws", "1.0.0", "linux", "amd64")
                .as_str(),
            "https://registry.terraform.io/v1/providers/hashicorp/aws/1.0.0/download/linux/amd64"
        );
    }
}
