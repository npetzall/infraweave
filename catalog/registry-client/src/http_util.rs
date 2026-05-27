//! Shared HTTP helpers for registry JSON requests (provider and module download).

use reqwest::header::{HeaderMap, ACCEPT, CONTENT_TYPE};
use reqwest::{Client, RequestBuilder};

use crate::error::ProviderRegistryError;

pub(crate) const JSON_MEDIA: &str = "application/json";

/// Returns true when the `Content-Type` primary type is `application/json`.
pub(crate) fn is_json_content_type(content_type: &str) -> bool {
    content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .eq_ignore_ascii_case(JSON_MEDIA)
}

pub(crate) fn check_json_content_type(
    url: &str,
    content_type: Option<&str>,
) -> Result<(), ProviderRegistryError> {
    let Some(ct) = content_type else {
        return Err(ProviderRegistryError::UnexpectedContentType {
            url: url.to_string(),
            content_type: None,
        });
    };
    if !is_json_content_type(ct) {
        return Err(ProviderRegistryError::UnexpectedContentType {
            url: url.to_string(),
            content_type: Some(ct.to_string()),
        });
    }
    Ok(())
}

pub(crate) fn apply_request_headers(
    builder: RequestBuilder,
    extra: Option<&HeaderMap>,
) -> RequestBuilder {
    let builder = builder.header(ACCEPT, JSON_MEDIA);
    if let Some(headers) = extra {
        builder.headers(headers.clone())
    } else {
        builder
    }
}

pub(crate) async fn read_json_body(
    response: reqwest::Response,
    request_url: &str,
) -> Result<String, ProviderRegistryError> {
    let status = response.status();
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let body = response
        .text()
        .await
        .map_err(|e| ProviderRegistryError::http(request_url, e))?;

    if !status.is_success() {
        return Err(ProviderRegistryError::UnsuccessfulStatus {
            url: request_url.to_string(),
            status,
        });
    }

    check_json_content_type(request_url, content_type.as_deref())?;
    Ok(body)
}

pub(crate) async fn fetch_json(
    http: &Client,
    url: &str,
    timeout: std::time::Duration,
    extra_headers: Option<&HeaderMap>,
) -> Result<(String, reqwest::Url), ProviderRegistryError> {
    let response = apply_request_headers(http.get(url), extra_headers)
        .timeout(timeout)
        .send()
        .await
        .map_err(|e| ProviderRegistryError::http(url, e))?;
    let final_url = response.url().clone();
    let body = read_json_body(response, final_url.as_str()).await?;
    Ok((body, final_url))
}

/// Rejects non-HTTPS artifact URLs unless the target host is loopback (tests / local registries).
pub(crate) fn require_https_artifact_url(url: &reqwest::Url) -> Result<(), ProviderRegistryError> {
    if url.scheme().eq_ignore_ascii_case("https") {
        return Ok(());
    }
    if url.scheme().eq_ignore_ascii_case("http") && is_loopback_url(url) {
        return Ok(());
    }
    Err(ProviderRegistryError::InsecureArtifactUrl {
        url: url.to_string(),
    })
}

pub(crate) fn is_loopback_url(url: &reqwest::Url) -> bool {
    match url.host() {
        Some(url::Host::Domain(host)) => host.eq_ignore_ascii_case("localhost"),
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_content_type_accepts_charset() {
        assert!(is_json_content_type("application/json; charset=utf-8"));
        assert!(!is_json_content_type("text/plain"));
    }

    #[test]
    fn check_json_content_type_rejects_plain() {
        let err = check_json_content_type("http://example/x", Some("text/plain")).unwrap_err();
        assert!(matches!(
            err,
            ProviderRegistryError::UnexpectedContentType {
                content_type: Some(ct),
                ..
            } if ct == "text/plain"
        ));
    }

    #[test]
    fn require_https_allows_loopback_http() {
        let u = reqwest::Url::parse("http://127.0.0.1:9/z.zip").unwrap();
        require_https_artifact_url(&u).unwrap();
    }

    #[test]
    fn require_https_rejects_remote_http() {
        let u = reqwest::Url::parse("http://releases.example/z.zip").unwrap();
        assert!(matches!(
            require_https_artifact_url(&u),
            Err(ProviderRegistryError::InsecureArtifactUrl { .. })
        ));
    }
}
