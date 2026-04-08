//! HTTP client for the provider registry API.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use futures_util::stream::{self, StreamExt};
use reqwest::Url;
use serde::de::DeserializeOwned;
use tokio::io::AsyncWriteExt;
use tracing::warn;

use crate::error::{PlatformDownloadError, ProviderRegistryError};
use crate::keyring::GpgKeyring;
use crate::registry::{PackageDownload, ProviderRegistry, Registry};
use crate::{FileArtifact, ProviderPackage};

/// Verified file artifacts for one platform, or why that platform was skipped.
pub type PlatformArtifactResult = Result<Vec<FileArtifact>, PlatformDownloadError>;

/// Map from platform id (e.g. `linux_amd64`) to downloaded artifact paths or per-platform error.
pub type DownloadReport = HashMap<String, PlatformArtifactResult>;

/// HTTP client bound to a registry host ([`Registry`]), mirroring [`Registry::provider`] for the resolved API client.
#[derive(Debug, Clone)]
pub struct RegistryClient {
    http: reqwest::Client,
    registry: Registry,
}

impl RegistryClient {
    fn default_client() -> Result<reqwest::Client, ProviderRegistryError> {
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

    pub fn new(registry: Registry) -> Result<Self, ProviderRegistryError> {
        let http = Self::default_client()?;
        Ok(Self { http, registry })
    }

    pub fn with_http_client(registry: Registry, http: reqwest::Client) -> Self {
        Self { http, registry }
    }

    /// Resolved providers API ([`ProviderRegistry`]) plus shared [`reqwest::Client`], same as [`Registry::provider`].
    pub async fn provider(&self) -> Result<ProviderRegistryClient, ProviderRegistryError> {
        let provider = self.registry.provider(&self.http).await?;
        Ok(ProviderRegistryClient::new(provider, self.http.clone()))
    }
}

/// HTTP client for the normalized provider registry REST API ([`ProviderRegistry`]).
#[derive(Debug, Clone)]
pub struct ProviderRegistryClient {
    http: reqwest::Client,
    provider: ProviderRegistry,
}

impl ProviderRegistryClient {
    pub(crate) fn new(provider: ProviderRegistry, http: reqwest::Client) -> Self {
        Self { http, provider }
    }

    /// Downloads each `platform` (e.g. `linux_amd64`) under `dir`, verifies artifacts, and returns a [`DownloadReport`](crate::DownloadReport):
    /// platform id → verified [`FileArtifact`] entries or per-platform error.
    ///
    /// At most five platforms download concurrently (each platform fetches three artifacts in parallel), so large-provider bandwidth stays bounded.
    ///
    /// Per-platform download or validation failures are logged and recorded in the map; the outer [`Result`] is only for whole-operation errors (e.g. staging directory).
    pub async fn download<T: AsRef<str>>(
        &self,
        namespace: &str,
        provider_type: &str,
        version: &str,
        platforms: &[T],
        dir: PathBuf,
    ) -> Result<DownloadReport, ProviderRegistryError> {
        if platforms.is_empty() {
            return Ok(HashMap::new());
        }

        tokio::fs::create_dir_all(&dir).await?;

        let ns = namespace.to_string();
        let pt = provider_type.to_string();
        let ver = version.to_string();
        let dir_clone = dir.clone();

        const MAX_CONCURRENT_PLATFORM_DOWNLOADS: usize = 5;

        // Own platform ids so this future does not borrow `platforms` across awaits (spawn-safe).
        let platform_list: Vec<String> = platforms.iter().map(|p| p.as_ref().to_string()).collect();

        let results: Vec<(String, Result<ProviderPackage, PlatformDownloadError>)> =
            stream::iter(platform_list.into_iter().map(|platform| {
                let c = self.clone();
                let ns = ns.clone();
                let pt = pt.clone();
                let ver = ver.clone();
                let dir = dir_clone.clone();
                async move {
                    let res = match c
                        .download_for_platform(&ns, &pt, &ver, &platform, dir)
                        .await
                    {
                        Ok(pkg) => Ok(pkg),
                        Err(e) => {
                            warn!(
                                platform = %platform,
                                error = %e,
                                "registry: download_for_platform failed"
                            );
                            Err(PlatformDownloadError::Download(e))
                        }
                    };
                    (platform, res)
                }
            }))
            .buffer_unordered(MAX_CONCURRENT_PLATFORM_DOWNLOADS)
            .collect()
            .await;

        let mut out = HashMap::with_capacity(results.len());
        for (platform, res) in results {
            match res {
                Err(e) => {
                    out.insert(platform, Err(e));
                }
                Ok(pkg) => match pkg.validate() {
                    Ok(entries) => {
                        out.insert(platform, Ok(entries));
                    }
                    Err(e) => {
                        warn!(
                            platform = %platform,
                            error = %e,
                            "registry: ProviderPackage::validate failed"
                        );
                        out.insert(platform, Err(PlatformDownloadError::Validate(e)));
                    }
                },
            }
        }

        Ok(out)
    }

    async fn download_for_platform(
        &self,
        namespace: &str,
        provider_type: &str,
        version: &str,
        platform: &str,
        dir: PathBuf,
    ) -> Result<ProviderPackage, ProviderRegistryError> {
        let (os, arch) = parse_platform(platform)?;
        let meta_url =
            self.provider
                .provider_package_url(namespace, provider_type, version, os, arch);
        let meta = self.get_json::<PackageDownload>(meta_url.as_str()).await?;
        let platform_dir = dir.join(platform);
        tokio::fs::create_dir_all(&platform_dir).await?;

        let provider = platform_dir.join(&meta.filename);
        let (shasums_file, sig_file, keyring_file) =
            sidecar_filenames_from_package_filename(&meta.filename);
        let shasums = platform_dir.join(&shasums_file);
        let signature = platform_dir.join(&sig_file);
        let keyring = platform_dir.join(&keyring_file);

        let c1 = self.clone();
        let c2 = self.clone();
        let c3 = self.clone();
        tokio::try_join!(
            c1.download_to_file(meta_url.clone(), &meta.download_url, provider.clone(), true,),
            c2.download_to_file(meta_url.clone(), &meta.shasums_url, shasums.clone(), false),
            c3.download_to_file(
                meta_url.clone(),
                &meta.shasums_signature_url,
                signature.clone(),
                false,
            ),
        )?;

        let mut gpg_keyring = GpgKeyring::new();
        for k in &meta.signing_keys.gpg_public_keys {
            gpg_keyring.add_armored(&k.ascii_armor);
        }
        tokio::fs::write(&keyring, gpg_keyring.to_file_contents().as_bytes()).await?;

        Ok(ProviderPackage {
            provider: FileArtifact {
                filename: meta.filename.clone(),
                path: provider,
            },
            shasum: meta.shasum.clone(),
            shasums: FileArtifact {
                filename: shasums_file,
                path: shasums,
            },
            signature: FileArtifact {
                filename: sig_file,
                path: signature,
            },
            keyring: FileArtifact {
                filename: keyring_file,
                path: keyring,
            },
        })
    }

    async fn get_json<T: DeserializeOwned>(&self, url: &str) -> Result<T, ProviderRegistryError> {
        let response = self
            .http
            .get(url)
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| ProviderRegistryError::http(url, e))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| ProviderRegistryError::http(url, e))?;

        if !status.is_success() {
            return Err(ProviderRegistryError::UnsuccessfulStatus {
                url: url.to_string(),
                status,
            });
        }

        serde_json::from_str(&body).map_err(|e| ProviderRegistryError::json(url, e))
    }

    async fn download_to_file(
        &self,
        base_url: Url,
        url: &str,
        file: PathBuf,
        chunked: bool,
    ) -> Result<(), ProviderRegistryError> {
        let url = match Url::parse(url) {
            Ok(parsed)
                if parsed.scheme().eq_ignore_ascii_case("http")
                    || parsed.scheme().eq_ignore_ascii_case("https") =>
            {
                parsed
            }
            _ => base_url
                .join(url)
                .map_err(|source| ProviderRegistryError::UrlResolve {
                    base: base_url.as_str().to_string(),
                    reference: url.to_string(),
                    source,
                })?,
        };
        let http = self.http.clone();
        let url_str = url.as_str().to_string();
        let response = http
            .get(url_str.as_str())
            .timeout(if chunked {
                Duration::from_secs(60)
            } else {
                Duration::from_secs(10)
            })
            .send()
            .await
            .map_err(|e| ProviderRegistryError::http(&url_str, e))?;

        let status = response.status();
        if !status.is_success() {
            return Err(ProviderRegistryError::UnsuccessfulStatus {
                url: url_str,
                status,
            });
        }

        if chunked {
            let mut file = tokio::fs::File::create(&file).await?;
            let mut stream = response.bytes_stream();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|e| ProviderRegistryError::http(&url_str, e))?;
                file.write_all(&chunk).await?;
            }
            file.flush().await?;
        } else {
            let bytes = response
                .bytes()
                .await
                .map_err(|e| ProviderRegistryError::http(&url_str, e))?;
            tokio::fs::write(&file, &bytes).await?;
        }
        Ok(())
    }
}

/// Sidecar files on disk: same stem as `package.filename`, with `.shasums`, `.shasums.sig`, `.shasums.asc`.
fn sidecar_filenames_from_package_filename(filename: &str) -> (String, String, String) {
    let stem = filename
        .rsplit_once('.')
        .map(|(s, _)| s)
        .unwrap_or(filename);
    (
        format!("{stem}.shasums"),
        format!("{stem}.shasums.sig"),
        format!("{stem}.shasums.asc"),
    )
}

fn parse_platform(platform: &str) -> Result<(&str, &str), ProviderRegistryError> {
    let parts: Vec<&str> = platform.split('_').collect();
    if parts.len() != 2 {
        return Err(ProviderRegistryError::InvalidPlatform {
            platform: platform.to_string(),
        });
    }
    Ok((parts[0], parts[1]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PlatformDownloadError;
    use tempfile::tempdir;

    #[test]
    fn sidecar_names_match_package_filename_stem() {
        let (sums, sig, keyring) = sidecar_filenames_from_package_filename(
            "terraform-provider-random_3.1.0_linux_amd64.zip",
        );
        assert_eq!(sums, "terraform-provider-random_3.1.0_linux_amd64.shasums");
        assert_eq!(
            sig,
            "terraform-provider-random_3.1.0_linux_amd64.shasums.sig"
        );
        assert_eq!(
            keyring,
            "terraform-provider-random_3.1.0_linux_amd64.shasums.asc"
        );
    }

    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn registry_builtin_terraform_no_network() {
        let r = Registry::new("registry.terraform.io");
        let c = RegistryClient::new(r).expect("client");
        let p = c.provider().await.expect("provider");
        let u = p
            .provider
            .provider_package_url("hashicorp", "aws", "1.0.0", "linux", "amd64");
        assert_eq!(
            u.as_str(),
            "https://registry.terraform.io/v1/providers/hashicorp/aws/1.0.0/download/linux/amd64"
        );
    }

    #[tokio::test]
    async fn registry_unknown_uses_well_known() {
        let server = MockServer::start().await;
        let port = server.address().port();
        let reg_host = format!("127.0.0.1:{port}");

        Mock::given(method("GET"))
            .and(path("/.well-known/terraform.json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "providers.v1": format!("http://127.0.0.1:{port}/reg/providers/")
            })))
            .mount(&server)
            .await;

        let r = Registry::new(reg_host.as_str());
        let client = RegistryClient::new(r).expect("client");
        let p = client.provider().await.expect("provider");
        let u = p
            .provider
            .provider_package_url("ns", "pty", "2.0.0", "linux", "amd64");
        assert_eq!(
            u.as_str(),
            format!("http://127.0.0.1:{port}/reg/providers/ns/pty/2.0.0/download/linux/amd64")
        );
    }

    #[tokio::test]
    async fn registry_discovery_missing_providers_v1() {
        let server = MockServer::start().await;
        let port = server.address().port();
        let reg_host = format!("127.0.0.1:{port}");

        Mock::given(method("GET"))
            .and(path("/.well-known/terraform.json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "modules.v1": "http://ignored/"
            })))
            .mount(&server)
            .await;

        let r = Registry::new(reg_host.as_str());
        let client = RegistryClient::new(r).expect("client");
        let err = client
            .provider()
            .await
            .expect_err("expected missing providers.v1");
        assert!(matches!(
            err,
            ProviderRegistryError::MissingProvidersV1 { .. }
        ));
    }

    #[tokio::test]
    async fn fetch_package_metadata_json() {
        let server = MockServer::start().await;
        let port = server.address().port();
        let reg_host = format!("127.0.0.1:{port}");

        Mock::given(method("GET"))
            .and(path("/.well-known/terraform.json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "providers.v1": format!("http://127.0.0.1:{port}/v1/providers")
            })))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path(
                "/v1/providers/hashicorp/random/3.1.0/download/linux/amd64",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "protocols": ["5.0"],
                "os": "linux",
                "arch": "amd64",
                "filename": "terraform-provider-random_3.1.0_linux_amd64.zip",
                "download_url": "https://releases.example/p.zip",
                "shasums_url": "https://releases.example/SHA256SUMS",
                "shasums_signature_url": "https://releases.example/SHA256SUMS.sig",
                "shasum": "abc",
                "signing_keys": { "gpg_public_keys": [] }
            })))
            .mount(&server)
            .await;

        let http = reqwest::Client::new();
        let registry = Registry::new(&reg_host);
        let client = RegistryClient::with_http_client(registry, http);
        let p = client.provider().await.expect("provider");

        let url = p
            .provider
            .provider_package_url("hashicorp", "random", "3.1.0", "linux", "amd64");
        let pkg = p
            .get_json::<PackageDownload>(url.as_str())
            .await
            .expect("pkg");
        assert!(pkg.download_url.contains("p.zip"));
    }

    #[tokio::test]
    async fn download_skips_when_validate_fails_empty_keys() {
        let server = MockServer::start().await;
        let port = server.address().port();
        let reg_host = format!("127.0.0.1:{port}");

        Mock::given(method("GET"))
            .and(path("/.well-known/terraform.json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "providers.v1": format!("http://127.0.0.1:{port}/v1/providers")
            })))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path(
                "/v1/providers/hashicorp/random/1.0.0/download/linux/amd64",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "protocols": ["5.0"],
                "os": "linux",
                "arch": "amd64",
                "filename": "p.zip",
                "download_url": format!("http://127.0.0.1:{port}/bins/amd64.zip"),
                "shasums_url": format!("http://127.0.0.1:{port}/SHA256SUMS"),
                "shasums_signature_url": format!("http://127.0.0.1:{port}/SHA256SUMS.sig"),
                "shasum": "abc",
                "signing_keys": { "gpg_public_keys": [] }
            })))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/bins/amd64.zip"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"zip-amd64"))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/SHA256SUMS"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"sums"))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/SHA256SUMS.sig"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"sig"))
            .mount(&server)
            .await;

        let http = reqwest::Client::new();
        let registry = Registry::new(&reg_host);
        let client = RegistryClient::with_http_client(registry, http);
        let p = client.provider().await.expect("provider");
        let dir = tempdir().expect("tempdir");

        let got = p
            .download(
                "hashicorp",
                "random",
                "1.0.0",
                &["linux_amd64"],
                dir.path().to_path_buf(),
            )
            .await
            .expect("download");

        let err = got
            .get("linux_amd64")
            .expect("platform entry")
            .as_ref()
            .expect_err("validation should fail with empty GPG keys");
        assert!(matches!(err, PlatformDownloadError::Validate(_)));
    }
}
