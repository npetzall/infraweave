use super::*;
use crate::client::RegistryClient;
use crate::registry::Registry;
use tempfile::tempdir;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn mount_discovery_both(server: &MockServer, port: u16) {
    Mock::given(method("GET"))
        .and(path("/.well-known/terraform.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "providers.v1": format!("http://127.0.0.1:{port}/v1/providers/"),
            "modules.v1": format!("http://127.0.0.1:{port}/v1/modules/")
        })))
        .mount(server)
        .await;
}

#[tokio::test]
async fn download_204_terraform_get() {
    let server = MockServer::start().await;
    let port = server.address().port();
    let reg_host = format!("127.0.0.1:{port}");
    mount_discovery_both(&server, port).await;

    Mock::given(method("GET"))
        .and(path("/v1/modules/ns/mod/sys/1.0.0/download"))
        .respond_with(ResponseTemplate::new(204).insert_header(
            "X-Terraform-Get",
            format!("http://127.0.0.1:{port}/release/mod.zip"),
        ))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/release/mod.zip"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"module-bytes"))
        .mount(&server)
        .await;

    let client = RegistryClient::with_http_client(Registry::new(&reg_host), reqwest::Client::new());
    let m = client.module().await.expect("module");
    let dir = tempdir().unwrap();
    let pkg = m
        .download("ns", "mod", "sys", "1.0.0", dir.path().to_path_buf())
        .await
        .expect("download");
    assert_eq!(std::fs::read(&pkg.archive.path).unwrap(), b"module-bytes");
}

#[tokio::test]
async fn download_200_json_location() {
    let server = MockServer::start().await;
    let port = server.address().port();
    let reg_host = format!("127.0.0.1:{port}");
    mount_discovery_both(&server, port).await;

    Mock::given(method("GET"))
        .and(path("/v1/modules/ns/mod/sys/2.0.0/download"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "location": format!("http://127.0.0.1:{port}/release/mod2.zip")
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/release/mod2.zip"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"v2"))
        .mount(&server)
        .await;

    let client = RegistryClient::with_http_client(Registry::new(&reg_host), reqwest::Client::new());
    let m = client.module().await.expect("module");
    let dir = tempdir().unwrap();
    let pkg = m
        .download("ns", "mod", "sys", "2.0.0", dir.path().to_path_buf())
        .await
        .expect("download");
    assert_eq!(std::fs::read(&pkg.archive.path).unwrap(), b"v2");
}

#[tokio::test]
async fn download_header_wins_over_body() {
    let server = MockServer::start().await;
    let port = server.address().port();
    let reg_host = format!("127.0.0.1:{port}");
    mount_discovery_both(&server, port).await;

    Mock::given(method("GET"))
        .and(path("/v1/modules/ns/mod/sys/3.0.0/download"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({
                    "location": format!("http://127.0.0.1:{port}/ignored.zip")
                }))
                .insert_header(
                    "X-Terraform-Get",
                    format!("http://127.0.0.1:{port}/release/header.zip"),
                ),
        )
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/release/header.zip"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"from-header"))
        .mount(&server)
        .await;

    let client = RegistryClient::with_http_client(Registry::new(&reg_host), reqwest::Client::new());
    let m = client.module().await.expect("module");
    let dir = tempdir().unwrap();
    let pkg = m
        .download("ns", "mod", "sys", "3.0.0", dir.path().to_path_buf())
        .await
        .expect("download");
    assert_eq!(std::fs::read(&pkg.archive.path).unwrap(), b"from-header");
}

#[tokio::test]
async fn download_json_wrong_content_type() {
    let server = MockServer::start().await;
    let port = server.address().port();
    let reg_host = format!("127.0.0.1:{port}");
    mount_discovery_both(&server, port).await;

    Mock::given(method("GET"))
        .and(path("/v1/modules/ns/mod/sys/4.0.0/download"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(r#"{"location":"http://127.0.0.1:1/x.zip"}"#)
                .insert_header("Content-Type", "text/html"),
        )
        .mount(&server)
        .await;

    let client = RegistryClient::with_http_client(Registry::new(&reg_host), reqwest::Client::new());
    let m = client.module().await.expect("module");
    let err = m
        .resolve_source_location("ns", "mod", "sys", "4.0.0")
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        ProviderRegistryError::UnexpectedContentType { .. }
    ));
}

#[tokio::test]
async fn download_missing_location() {
    let server = MockServer::start().await;
    let port = server.address().port();
    let reg_host = format!("127.0.0.1:{port}");
    mount_discovery_both(&server, port).await;

    Mock::given(method("GET"))
        .and(path("/v1/modules/ns/mod/sys/5.0.0/download"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let client = RegistryClient::with_http_client(Registry::new(&reg_host), reqwest::Client::new());
    let m = client.module().await.expect("module");
    let err = m
        .resolve_source_location("ns", "mod", "sys", "5.0.0")
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        ProviderRegistryError::MissingModuleSourceLocation { .. }
    ));
}

#[tokio::test]
async fn download_unsupported_git_scheme() {
    let server = MockServer::start().await;
    let port = server.address().port();
    let reg_host = format!("127.0.0.1:{port}");
    mount_discovery_both(&server, port).await;

    Mock::given(method("GET"))
        .and(path("/v1/modules/ns/mod/sys/6.0.0/download"))
        .respond_with(ResponseTemplate::new(204).insert_header(
            "X-Terraform-Get",
            "git::https://github.com/example/module.git",
        ))
        .mount(&server)
        .await;

    let client = RegistryClient::with_http_client(Registry::new(&reg_host), reqwest::Client::new());
    let m = client.module().await.expect("module");
    let dir = tempdir().unwrap();
    let err = m
        .download("ns", "mod", "sys", "6.0.0", dir.path().to_path_buf())
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        ProviderRegistryError::UnsupportedModuleSourceScheme { .. }
    ));
}

#[tokio::test]
async fn download_relative_location() {
    let server = MockServer::start().await;
    let port = server.address().port();
    let reg_host = format!("127.0.0.1:{port}");
    mount_discovery_both(&server, port).await;

    Mock::given(method("GET"))
        .and(path("/v1/modules/ns/mod/sys/7.0.0/download"))
        .respond_with(
            ResponseTemplate::new(204).insert_header("X-Terraform-Get", "release/rel.zip"),
        )
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/v1/modules/ns/mod/sys/7.0.0/release/rel.zip"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"rel"))
        .mount(&server)
        .await;

    let client = RegistryClient::with_http_client(Registry::new(&reg_host), reqwest::Client::new());
    let m = client.module().await.expect("module");
    let dir = tempdir().unwrap();
    let pkg = m
        .download("ns", "mod", "sys", "7.0.0", dir.path().to_path_buf())
        .await
        .expect("download");
    assert_eq!(std::fs::read(&pkg.archive.path).unwrap(), b"rel");
}

#[tokio::test]
async fn download_artifact_non_success() {
    let server = MockServer::start().await;
    let port = server.address().port();
    let reg_host = format!("127.0.0.1:{port}");
    mount_discovery_both(&server, port).await;

    Mock::given(method("GET"))
        .and(path("/v1/modules/ns/mod/sys/8.0.0/download"))
        .respond_with(ResponseTemplate::new(204).insert_header(
            "X-Terraform-Get",
            format!("http://127.0.0.1:{port}/missing.zip"),
        ))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/missing.zip"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let client = RegistryClient::with_http_client(Registry::new(&reg_host), reqwest::Client::new());
    let m = client.module().await.expect("module");
    let dir = tempdir().unwrap();
    let err = m
        .download("ns", "mod", "sys", "8.0.0", dir.path().to_path_buf())
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        ProviderRegistryError::UnsuccessfulStatus { .. }
    ));
}

#[tokio::test]
async fn download_with_request_headers() {
    let server = MockServer::start().await;
    let port = server.address().port();
    let reg_host = format!("127.0.0.1:{port}");

    Mock::given(method("GET"))
        .and(path("/.well-known/terraform.json"))
        .and(header("authorization", "Bearer secret"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "modules.v1": format!("http://127.0.0.1:{port}/v1/modules/")
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/v1/modules/ns/mod/sys/9.0.0/download"))
        .and(header("authorization", "Bearer secret"))
        .respond_with(ResponseTemplate::new(204).insert_header(
            "X-Terraform-Get",
            format!("http://127.0.0.1:{port}/auth.zip"),
        ))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/auth.zip"))
        .and(header("authorization", "Bearer secret"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"auth"))
        .mount(&server)
        .await;

    let mut headers = HeaderMap::new();
    headers.insert(
        reqwest::header::AUTHORIZATION,
        reqwest::header::HeaderValue::from_static("Bearer secret"),
    );
    let client = RegistryClient::with_http_client(Registry::new(&reg_host), reqwest::Client::new())
        .with_request_headers(headers);
    let m = client.module().await.expect("module");
    let dir = tempdir().unwrap();
    let pkg = m
        .download("ns", "mod", "sys", "9.0.0", dir.path().to_path_buf())
        .await
        .expect("download");
    assert_eq!(std::fs::read(&pkg.archive.path).unwrap(), b"auth");
}
