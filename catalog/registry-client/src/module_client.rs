//! HTTP client for the module registry download API.

use std::path::{Path, PathBuf};
use std::time::Duration;

use reqwest::header::{HeaderMap, CONTENT_DISPOSITION};
use reqwest::StatusCode;
use reqwest::Url;
use tokio::io::AsyncWriteExt;

use crate::error::ProviderRegistryError;
use crate::http_util::{apply_request_headers, read_json_body, require_https_artifact_url};
use crate::module_registry::{ModuleDownloadFallback, ModuleRegistry};
use crate::provider_package::FileArtifact;

const HEADER_TERRAFORM_GET: &str = "x-terraform-get";

/// On-disk module archive after download.
#[derive(Debug, Clone)]
pub struct ModulePackage {
    pub archive: FileArtifact,
    /// Resolved source location from the registry (before artifact fetch).
    pub source_location: String,
}

/// HTTP client for the normalized module registry REST API ([`ModuleRegistry`]).
#[derive(Debug, Clone)]
pub struct ModuleRegistryClient {
    http: reqwest::Client,
    module: ModuleRegistry,
    request_headers: Option<HeaderMap>,
}

impl ModuleRegistryClient {
    pub(crate) fn new(
        module: ModuleRegistry,
        http: reqwest::Client,
        request_headers: Option<HeaderMap>,
    ) -> Self {
        Self {
            http,
            module,
            request_headers,
        }
    }

    /// Returns the resolved modules API base.
    pub fn module_registry(&self) -> &ModuleRegistry {
        &self.module
    }

    /// Resolves the module source location from the registry download endpoint.
    pub async fn resolve_source_location(
        &self,
        namespace: &str,
        name: &str,
        system: &str,
        version: &str,
    ) -> Result<(String, Url), ProviderRegistryError> {
        let download_url = self
            .module
            .module_download_url(namespace, name, system, version);
        let response = apply_request_headers(
            self.http.get(download_url.as_str()),
            self.request_headers.as_ref(),
        )
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| ProviderRegistryError::http(download_url.as_str(), e))?;

        let final_url = response.url().clone();
        let status = response.status();
        let request_url = final_url.as_str();

        if status == StatusCode::NO_CONTENT {
            let location = terraform_get_header(response.headers())?.ok_or_else(|| {
                ProviderRegistryError::MissingModuleSourceLocation {
                    url: request_url.to_string(),
                }
            })?;
            return Ok((location, final_url));
        }

        if status.is_success() {
            if let Some(location) = terraform_get_header(response.headers())? {
                return Ok((location, final_url));
            }
            let body = read_json_body(response, request_url).await?;
            let fallback: ModuleDownloadFallback = serde_json::from_str(&body)
                .map_err(|e| ProviderRegistryError::json(request_url, e))?;
            let location = fallback.location.trim();
            if location.is_empty() {
                return Err(ProviderRegistryError::MissingModuleSourceLocation {
                    url: request_url.to_string(),
                });
            }
            return Ok((location.to_string(), final_url));
        }

        Err(ProviderRegistryError::UnsuccessfulStatus {
            url: request_url.to_string(),
            status,
        })
    }

    /// Downloads a module archive for a known version when the source location is `http://` or `https://`.
    pub async fn download(
        &self,
        namespace: &str,
        name: &str,
        system: &str,
        version: &str,
        dir: PathBuf,
    ) -> Result<ModulePackage, ProviderRegistryError> {
        let (location, base_url) = self
            .resolve_source_location(namespace, name, system, version)
            .await?;
        let artifact_url = resolve_module_source_url(&location, &base_url)?;
        require_https_artifact_url(&artifact_url)?;

        tokio::fs::create_dir_all(&dir).await?;

        let filename = artifact_filename(&artifact_url, None);
        let path = dir.join(&filename);

        let written = self.download_artifact(&artifact_url, &path).await?;
        let filename = written
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(&filename)
            .to_string();

        Ok(ModulePackage {
            archive: FileArtifact {
                filename,
                path: written,
            },
            source_location: location,
        })
    }

    async fn download_artifact(
        &self,
        url: &Url,
        file: &Path,
    ) -> Result<PathBuf, ProviderRegistryError> {
        let url_str = url.as_str();
        let response = apply_request_headers(self.http.get(url_str), self.request_headers.as_ref())
            .timeout(Duration::from_secs(60))
            .send()
            .await
            .map_err(|e| ProviderRegistryError::http(url_str, e))?;

        let status = response.status();
        if !status.is_success() {
            return Err(ProviderRegistryError::UnsuccessfulStatus {
                url: url_str.to_string(),
                status,
            });
        }

        let filename_from_header = response
            .headers()
            .get(CONTENT_DISPOSITION)
            .and_then(|v| v.to_str().ok())
            .and_then(parse_content_disposition_filename);

        let mut out_path = file.to_path_buf();
        if let Some(name) = filename_from_header {
            if let Some(parent) = file.parent() {
                out_path = parent.join(name);
            }
        }

        let mut f = tokio::fs::File::create(&out_path).await?;
        let mut stream = response.bytes_stream();
        use futures_util::StreamExt;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| ProviderRegistryError::http(url_str, e))?;
            f.write_all(&chunk).await?;
        }
        f.flush().await?;
        Ok(out_path)
    }
}

fn terraform_get_header(headers: &HeaderMap) -> Result<Option<String>, ProviderRegistryError> {
    for (name, value) in headers.iter() {
        if name.as_str().eq_ignore_ascii_case(HEADER_TERRAFORM_GET) {
            let s = value
                .to_str()
                .map_err(|_| ProviderRegistryError::MissingModuleSourceLocation {
                    url: "<invalid X-Terraform-Get header>".into(),
                })?
                .trim();
            if s.is_empty() {
                return Ok(None);
            }
            return Ok(Some(s.to_string()));
        }
    }
    Ok(None)
}

fn resolve_module_source_url(location: &str, base_url: &Url) -> Result<Url, ProviderRegistryError> {
    let trimmed = location.trim();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return Url::parse(trimmed).map_err(ProviderRegistryError::InvalidBaseUrl);
    }
    if trimmed.contains("://") {
        return Err(ProviderRegistryError::UnsupportedModuleSourceScheme {
            location: trimmed.to_string(),
        });
    }
    base_url
        .join(trimmed)
        .map_err(|source| ProviderRegistryError::UrlResolve {
            base: base_url.to_string(),
            reference: trimmed.to_string(),
            source,
        })
}

fn artifact_filename(url: &Url, content_disposition: Option<&str>) -> String {
    if let Some(name) = content_disposition.and_then(parse_content_disposition_filename) {
        return name;
    }
    url.path_segments()
        .and_then(|mut s| s.next_back())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "module.zip".to_string())
}

fn parse_content_disposition_filename(value: &str) -> Option<String> {
    for part in value.split(';') {
        let part = part.trim();
        if let Some(rest) = part.strip_prefix("filename=") {
            let name = rest.trim().trim_matches('"');
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
#[path = "module_client_test.rs"]
mod tests;
