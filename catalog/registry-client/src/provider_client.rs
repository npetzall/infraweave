//! HTTP client for the provider registry download API.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use futures_util::stream::{self, StreamExt};
use reqwest::header::HeaderMap;
use reqwest::Url;
use tokio::io::AsyncWriteExt;
use tracing::warn;

use crate::error::{PlatformDownloadError, ProviderRegistryError};
use crate::http_util::{fetch_json, require_https_artifact_url};
use crate::keyring::GpgKeyring;
use crate::provider_registry::{PackageDownload, ProviderRegistry};
use crate::{FileArtifact, ProviderPackage};

/// Verified file artifacts for one platform, or why that platform was skipped.
pub type PlatformArtifactResult = Result<Vec<FileArtifact>, PlatformDownloadError>;

/// Map from platform id (e.g. `linux_amd64`) to downloaded artifact paths or per-platform error.
pub type DownloadReport = HashMap<String, PlatformArtifactResult>;

/// HTTP client for the normalized provider registry REST API ([`ProviderRegistry`]).
#[derive(Debug, Clone)]
pub struct ProviderRegistryClient {
    http: reqwest::Client,
    provider: ProviderRegistry,
    request_headers: Option<HeaderMap>,
}

impl ProviderRegistryClient {
    pub(crate) fn new(
        provider: ProviderRegistry,
        http: reqwest::Client,
        request_headers: Option<HeaderMap>,
    ) -> Self {
        Self {
            http,
            provider,
            request_headers,
        }
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
        let meta = self.fetch_package_metadata(&meta_url, os, arch).await?;
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

    async fn fetch_package_metadata(
        &self,
        meta_url: &Url,
        os: &str,
        arch: &str,
    ) -> Result<PackageDownload, ProviderRegistryError> {
        let (body, final_url) = fetch_json(
            &self.http,
            meta_url.as_str(),
            Duration::from_secs(10),
            self.request_headers.as_ref(),
        )
        .await?;
        let meta: PackageDownload = serde_json::from_str(&body)
            .map_err(|e| ProviderRegistryError::json(final_url.as_str(), e))?;
        meta.validate_for_platform(os, arch, final_url.as_str())?;
        Ok(meta)
    }

    #[cfg(test)]
    async fn get_json<T: serde::de::DeserializeOwned>(
        &self,
        url: &str,
    ) -> Result<T, ProviderRegistryError> {
        let (body, final_url) = fetch_json(
            &self.http,
            url,
            Duration::from_secs(10),
            self.request_headers.as_ref(),
        )
        .await?;
        serde_json::from_str(&body).map_err(|e| ProviderRegistryError::json(final_url.as_str(), e))
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
        require_https_artifact_url(&url)?;
        let http = self.http.clone();
        let url_str = url.as_str().to_string();
        let response = crate::http_util::apply_request_headers(
            http.get(url_str.as_str()),
            self.request_headers.as_ref(),
        )
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
#[path = "provider_client_test.rs"]
mod tests;
