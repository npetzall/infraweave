//! Module registry REST API base URLs and download response types.

use reqwest::Url;

/// Module registry REST API base (`…/v1/modules` or discovered equivalent).
#[derive(Debug, Clone)]
pub struct ModuleRegistry {
    base: Url,
}

impl ModuleRegistry {
    /// Resolved modules API base (discovery may include a trailing `/` on the path).
    pub fn base_url(&self) -> &Url {
        &self.base
    }

    pub(crate) fn from_base(base: Url) -> Self {
        Self { base }
    }

    /// `GET …/{namespace}/{name}/{system}/{version}/download`
    pub fn module_download_url(
        &self,
        namespace: &str,
        name: &str,
        system: &str,
        version: &str,
    ) -> Url {
        let mut u = self.base.clone();
        u.path_segments_mut()
            .expect("module registry base URL must support path segments")
            .pop_if_empty()
            .extend([namespace, name, system, version, "download"]);
        u
    }
}

/// JSON fallback for `GET …/download` when `X-Terraform-Get` is absent.
#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct ModuleDownloadFallback {
    pub location: String,
}

#[cfg(test)]
mod tests {
    use crate::registry::Registry;

    #[tokio::test]
    async fn module_registry_urls_terraform_builtin() {
        let reg = Registry::new("registry.terraform.io");
        let http = reqwest::Client::new();
        let mr = reg.module(&http).await.expect("module");
        assert_eq!(
            mr.module_download_url("hashicorp", "consul", "aws", "0.0.1")
                .as_str(),
            "https://registry.terraform.io/v1/modules/hashicorp/consul/aws/0.0.1/download"
        );
    }

    #[tokio::test]
    async fn module_registry_urls_opentofu_builtin() {
        let reg = Registry::new("registry.opentofu.org");
        let http = reqwest::Client::new();
        let mr = reg.module(&http).await.expect("module");
        assert_eq!(
            mr.module_download_url("terraform-aws-modules", "vpc", "aws", "5.0.0")
                .as_str(),
            "https://registry.opentofu.org/v1/modules/terraform-aws-modules/vpc/aws/5.0.0/download"
        );
    }
}
