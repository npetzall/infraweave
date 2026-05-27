//! Provider registry REST API base URLs and download metadata types.

use reqwest::Url;

use crate::error::ProviderRegistryError;

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

    pub(crate) fn from_base(base: Url) -> Self {
        Self { base }
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

impl PackageDownload {
    pub(crate) fn validate_for_platform(
        &self,
        os: &str,
        arch: &str,
        url: &str,
    ) -> Result<(), ProviderRegistryError> {
        if self.signing_keys.gpg_public_keys.is_empty() {
            return Err(ProviderRegistryError::NoGpgPublicKeys);
        }
        if !self.os.eq_ignore_ascii_case(os) || !self.arch.eq_ignore_ascii_case(arch) {
            return Err(ProviderRegistryError::PlatformMetadataMismatch {
                url: url.to_string(),
                expected_os: os.to_string(),
                expected_arch: arch.to_string(),
                got_os: self.os.clone(),
                got_arch: self.arch.clone(),
            });
        }
        Ok(())
    }
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

#[cfg(test)]
mod tests {
    use crate::registry::Registry;

    #[tokio::test]
    async fn provider_registry_urls_terraform_builtin() {
        let reg = Registry::new("registry.terraform.io");
        let http = reqwest::Client::new();
        let pr = reg.provider(&http).await.expect("provider");
        assert_eq!(
            pr.provider_package_url("hashicorp", "aws", "1.0.0", "linux", "amd64")
                .as_str(),
            "https://registry.terraform.io/v1/providers/hashicorp/aws/1.0.0/download/linux/amd64"
        );
    }

    #[tokio::test]
    async fn provider_registry_urls_opentofu_builtin() {
        let reg = Registry::new("registry.opentofu.org");
        let http = reqwest::Client::new();
        let pr = reg.provider(&http).await.expect("provider");
        assert_eq!(
            pr.provider_package_url("hashicorp", "null", "3.2.0", "linux", "amd64")
                .as_str(),
            "https://registry.opentofu.org/v1/providers/hashicorp/null/3.2.0/download/linux/amd64"
        );
    }
}
