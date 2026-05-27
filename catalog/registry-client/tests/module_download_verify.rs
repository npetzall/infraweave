//! Integration test: wiremock module registry + release server; client downloads module bytes.

use registry_client::{Registry, RegistryClient};
use tempfile::tempdir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const NS: &str = "acme";
const NAME: &str = "vpc";
const SYSTEM: &str = "aws";
const VERSION: &str = "1.0.0";
const ZIP_BYTES: &[u8] = b"synthetic module archive for integration test";

#[tokio::test]
async fn module_download_end_to_end() {
    let server = MockServer::start().await;
    let port = server.address().port();
    let base = format!("http://127.0.0.1:{port}");
    let reg_host = format!("127.0.0.1:{port}");

    Mock::given(method("GET"))
        .and(path("/.well-known/terraform.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "modules.v1": format!("{base}/v1/modules/")
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path(format!(
            "/v1/modules/{NS}/{NAME}/{SYSTEM}/{VERSION}/download"
        )))
        .respond_with(
            ResponseTemplate::new(204)
                .insert_header("X-Terraform-Get", format!("{base}/release/module.zip")),
        )
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/release/module.zip"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(ZIP_BYTES))
        .mount(&server)
        .await;

    let http = reqwest::Client::new();
    let client = RegistryClient::with_http_client(Registry::new(reg_host), http);
    let module_client = client.module().await.expect("module discovery");

    let dir = tempdir().expect("tempdir");
    let pkg = module_client
        .download(NS, NAME, SYSTEM, VERSION, dir.path().to_path_buf())
        .await
        .expect("download");

    assert_eq!(
        std::fs::read(&pkg.archive.path).expect("read archive"),
        ZIP_BYTES
    );
    assert!(pkg.source_location.contains("module.zip"));
}
