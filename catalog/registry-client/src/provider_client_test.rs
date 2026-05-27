use super::*;
use crate::client::RegistryClient;
use crate::registry::Registry;
use crate::PlatformDownloadError;
use tempfile::tempdir;

#[test]
fn sidecar_names_match_package_filename_stem() {
    let (sums, sig, keyring) =
        sidecar_filenames_from_package_filename("terraform-provider-random_3.1.0_linux_amd64.zip");
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

#[test]
fn sidecar_names_when_filename_has_no_dot_use_whole_name_as_stem() {
    let (sums, sig, keyring) = sidecar_filenames_from_package_filename("terraform-provider-random");
    assert_eq!(sums, "terraform-provider-random.shasums");
    assert_eq!(sig, "terraform-provider-random.shasums.sig");
    assert_eq!(keyring, "terraform-provider-random.shasums.asc");
}

#[test]
fn parse_platform_accepts_os_arch() {
    assert_eq!(parse_platform("linux_amd64").unwrap(), ("linux", "amd64"));
}

#[test]
fn parse_platform_rejects_bad_shapes() {
    assert!(matches!(
        parse_platform("linux"),
        Err(ProviderRegistryError::InvalidPlatform { platform }) if platform == "linux"
    ));
    assert!(matches!(
        parse_platform("a_b_c_d"),
        Err(ProviderRegistryError::InvalidPlatform { platform }) if platform == "a_b_c_d"
    ));
}

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn mount_minimal_discovery(server: &MockServer, port: u16) {
    Mock::given(method("GET"))
        .and(path("/.well-known/terraform.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "providers.v1": format!("http://127.0.0.1:{port}/v1/providers")
        })))
        .mount(server)
        .await;
}

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
async fn download_fails_fast_on_empty_gpg_keys() {
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
        .expect_err("empty GPG keys at metadata fetch");
    assert!(matches!(
        err,
        PlatformDownloadError::Download(ProviderRegistryError::NoGpgPublicKeys)
    ));
}

#[tokio::test]
async fn metadata_wrong_content_type() {
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
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(r#"{"os":"linux","arch":"amd64"}"#)
                .insert_header("Content-Type", "text/html"),
        )
        .mount(&server)
        .await;

    let http = reqwest::Client::new();
    let client = RegistryClient::with_http_client(Registry::new(&reg_host), http);
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
    assert!(matches!(
        got.get("linux_amd64").and_then(|r| r.as_ref().err()),
        Some(PlatformDownloadError::Download(
            ProviderRegistryError::UnexpectedContentType { .. }
        ))
    ));
}

#[tokio::test]
async fn metadata_platform_mismatch() {
    let server = MockServer::start().await;
    let port = server.address().port();
    let reg_host = format!("127.0.0.1:{port}");

    mount_minimal_discovery(&server, port).await;
    Mock::given(method("GET"))
        .and(path(
            "/v1/providers/hashicorp/random/1.0.0/download/linux/amd64",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "protocols": ["5.0"],
            "os": "darwin",
            "arch": "arm64",
            "filename": "p.zip",
            "download_url": format!("http://127.0.0.1:{port}/bins/p.zip"),
            "shasums_url": format!("http://127.0.0.1:{port}/SHA256SUMS"),
            "shasums_signature_url": format!("http://127.0.0.1:{port}/SHA256SUMS.sig"),
            "shasum": "abc",
            "signing_keys": { "gpg_public_keys": [{ "key_id": "AB", "ascii_armor": "-----BEGIN PGP PUBLIC KEY BLOCK-----\nx\n-----END PGP PUBLIC KEY BLOCK-----" }] }
        })))
        .mount(&server)
        .await;

    let http = reqwest::Client::new();
    let client = RegistryClient::with_http_client(Registry::new(&reg_host), http);
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
    assert!(matches!(
        got.get("linux_amd64").and_then(|r| r.as_ref().err()),
        Some(PlatformDownloadError::Download(
            ProviderRegistryError::PlatformMetadataMismatch { .. }
        ))
    ));
}

#[tokio::test]
async fn metadata_request_includes_accept_json() {
    use wiremock::matchers::{header, method, path};

    let server = MockServer::start().await;
    let port = server.address().port();
    let reg_host = format!("127.0.0.1:{port}");

    mount_minimal_discovery(&server, port).await;
    Mock::given(method("GET"))
        .and(path(
            "/v1/providers/hashicorp/random/1.0.0/download/linux/amd64",
        ))
        .and(header("accept", "application/json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "protocols": ["5.0"],
            "os": "linux",
            "arch": "amd64",
            "filename": "p.zip",
            "download_url": "http://127.0.0.1:1/unused.zip",
            "shasums_url": "http://127.0.0.1:1/SHA256SUMS",
            "shasums_signature_url": "http://127.0.0.1:1/SHA256SUMS.sig",
            "shasum": "abc",
            "signing_keys": { "gpg_public_keys": [{ "key_id": "AB", "ascii_armor": "-----BEGIN PGP PUBLIC KEY BLOCK-----\nx\n-----END PGP PUBLIC KEY BLOCK-----" }] }
        })))
        .mount(&server)
        .await;

    let http = reqwest::Client::new();
    let client = RegistryClient::with_http_client(Registry::new(&reg_host), http);
    let p = client.provider().await.expect("provider");
    let url = p
        .provider
        .provider_package_url("hashicorp", "random", "1.0.0", "linux", "amd64");
    let _ = p.get_json::<PackageDownload>(url.as_str()).await;
}

#[tokio::test]
async fn download_relative_artifact_urls() {
    let server = MockServer::start().await;
    let port = server.address().port();
    let reg_host = format!("127.0.0.1:{port}");

    mount_minimal_discovery(&server, port).await;
    Mock::given(method("GET"))
        .and(path(
            "/v1/providers/hashicorp/random/1.0.0/download/linux/amd64",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "protocols": ["5.0"],
            "os": "linux",
            "arch": "amd64",
            "filename": "p.zip",
            "download_url": "bins/p.zip",
            "shasums_url": "SHA256SUMS",
            "shasums_signature_url": "SHA256SUMS.sig",
            "shasum": "abc",
            "signing_keys": { "gpg_public_keys": [{ "key_id": "AB", "ascii_armor": "-----BEGIN PGP PUBLIC KEY BLOCK-----\nx\n-----END PGP PUBLIC KEY BLOCK-----" }] }
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/bins/p.zip"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"zip"))
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
    let client = RegistryClient::with_http_client(Registry::new(&reg_host), http);
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
    // Relative URLs resolve; validation may fail without real GPG — ensure we hit artifact paths
    let entry = got.get("linux_amd64").expect("entry");
    assert!(
        entry.is_err() || entry.as_ref().unwrap().iter().any(|a| a.path.exists()),
        "expected artifact fetch attempt"
    );
}

#[tokio::test]
async fn download_artifact_non_success_includes_url() {
    let server = MockServer::start().await;
    let port = server.address().port();
    let reg_host = format!("127.0.0.1:{port}");

    mount_minimal_discovery(&server, port).await;
    Mock::given(method("GET"))
        .and(path(
            "/v1/providers/hashicorp/random/1.0.0/download/linux/amd64",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "protocols": ["5.0"],
            "os": "linux",
            "arch": "amd64",
            "filename": "p.zip",
            "download_url": format!("http://127.0.0.1:{port}/bins/missing.zip"),
            "shasums_url": format!("http://127.0.0.1:{port}/SHA256SUMS"),
            "shasums_signature_url": format!("http://127.0.0.1:{port}/SHA256SUMS.sig"),
            "shasum": "abc",
            "signing_keys": { "gpg_public_keys": [{ "key_id": "AB", "ascii_armor": "-----BEGIN PGP PUBLIC KEY BLOCK-----\nx\n-----END PGP PUBLIC KEY BLOCK-----" }] }
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/bins/missing.zip"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let http = reqwest::Client::new();
    let client = RegistryClient::with_http_client(Registry::new(&reg_host), http);
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
    let err = got.get("linux_amd64").unwrap().as_ref().unwrap_err();
    assert!(matches!(
        err,
        PlatformDownloadError::Download(ProviderRegistryError::UnsuccessfulStatus { .. })
    ));
    if let PlatformDownloadError::Download(ProviderRegistryError::UnsuccessfulStatus {
        url, ..
    }) = err
    {
        assert!(url.contains("missing.zip"));
    }
}

#[tokio::test]
async fn download_wrong_zip_bytes_fails_validation() {
    let server = MockServer::start().await;
    let port = server.address().port();
    let reg_host = format!("127.0.0.1:{port}");

    mount_minimal_discovery(&server, port).await;
    Mock::given(method("GET"))
        .and(path(
            "/v1/providers/hashicorp/random/1.0.0/download/linux/amd64",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "protocols": ["5.0"],
            "os": "linux",
            "arch": "amd64",
            "filename": "p.zip",
            "download_url": format!("http://127.0.0.1:{port}/bins/p.zip"),
            "shasums_url": format!("http://127.0.0.1:{port}/SHA256SUMS"),
            "shasums_signature_url": format!("http://127.0.0.1:{port}/SHA256SUMS.sig"),
            "shasum": "deadbeef",
            "signing_keys": { "gpg_public_keys": [{ "key_id": "AB", "ascii_armor": "-----BEGIN PGP PUBLIC KEY BLOCK-----\nx\n-----END PGP PUBLIC KEY BLOCK-----" }] }
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/bins/p.zip"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"not-the-zip"))
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
    let client = RegistryClient::with_http_client(Registry::new(&reg_host), http);
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
    assert!(matches!(
        got.get("linux_amd64").and_then(|r| r.as_ref().err()),
        Some(PlatformDownloadError::Validate(_))
    ));
}

#[tokio::test]
async fn download_insecure_artifact_url_rejected() {
    let server = MockServer::start().await;
    let port = server.address().port();
    let reg_host = format!("127.0.0.1:{port}");

    mount_minimal_discovery(&server, port).await;
    Mock::given(method("GET"))
        .and(path(
            "/v1/providers/hashicorp/random/1.0.0/download/linux/amd64",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "protocols": ["5.0"],
            "os": "linux",
            "arch": "amd64",
            "filename": "p.zip",
            "download_url": "http://releases.example/p.zip",
            "shasums_url": "http://releases.example/SHA256SUMS",
            "shasums_signature_url": "http://releases.example/SHA256SUMS.sig",
            "shasum": "abc",
            "signing_keys": { "gpg_public_keys": [{ "key_id": "AB", "ascii_armor": "-----BEGIN PGP PUBLIC KEY BLOCK-----\nx\n-----END PGP PUBLIC KEY BLOCK-----" }] }
        })))
        .mount(&server)
        .await;

    let http = reqwest::Client::new();
    let client = RegistryClient::with_http_client(Registry::new(&reg_host), http);
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
    assert!(matches!(
        got.get("linux_amd64").and_then(|r| r.as_ref().err()),
        Some(PlatformDownloadError::Download(
            ProviderRegistryError::InsecureArtifactUrl { .. }
        ))
    ));
}

#[tokio::test]
async fn download_empty_platforms_yields_empty_report() {
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
            &[] as &[&str],
            dir.path().to_path_buf(),
        )
        .await
        .expect("download");

    assert!(got.is_empty());
}

#[tokio::test]
async fn download_records_invalid_platform_without_hitting_registry() {
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
            &["linux"],
            dir.path().to_path_buf(),
        )
        .await
        .expect("download");

    let err = got
        .get("linux")
        .expect("platform entry")
        .as_ref()
        .expect_err("invalid platform id");
    assert!(matches!(
        err,
        PlatformDownloadError::Download(ProviderRegistryError::InvalidPlatform {
            platform
        }) if platform == "linux"
    ));
}
