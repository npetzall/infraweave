//! Integration test: wiremock serves in-memory registry metadata and release artifacts; the client
//! writes downloads into a temp directory; on-disk bytes match the mock payloads and validation succeeds.

use std::io::{Cursor, Write};

use pgp::composed::{
    ArmorOptions, DetachedSignature, KeyType, SecretKeyParamsBuilder, SignedSecretKey,
};
use pgp::crypto::hash::HashAlgorithm;
use pgp::types::{KeyDetails, Password};
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use registry_client::{FileArtifact, PlatformDownloadError, Registry, RegistryClient};
use sha2::{Digest, Sha256};
use tempfile::tempdir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};
use zip::write::FileOptions;
use zip::ZipWriter;

const NS: &str = "acme";
const PROVIDER: &str = "null";
const VERSION: &str = "1.0.0";
const PLATFORM: &str = "linux_amd64";
const ZIP_NAME: &str = "terraform-provider-null_1.0.0_linux_amd64.zip";

fn build_zip_bytes() -> Vec<u8> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file("terraform-provider-null_v1.0.0", FileOptions::default())
        .expect("start_file");
    zip.write_all(b"synthetic provider payload for tests")
        .expect("write_all");
    zip.finish()
        .expect("finish")
        .into_inner()
}

fn generate_signing_key(rng: &mut ChaCha8Rng) -> SignedSecretKey {
    let params = SecretKeyParamsBuilder::default()
        .key_type(KeyType::Ed25519)
        .can_sign(true)
        .primary_user_id("registry-client integration <itest@example.invalid>".into())
        .build()
        .expect("key params");
    params.generate(rng).expect("generate key")
}

fn sha256_hex(data: &[u8]) -> String {
    let d = Sha256::digest(data);
    d.iter().map(|b| format!("{b:02x}")).collect()
}

#[tokio::test]
async fn download_and_verify_against_in_memory_mock() {
    let mut rng = ChaCha8Rng::seed_from_u64(0xC0FFEE);
    let secret = generate_signing_key(&mut rng);
    let public = secret.to_public_key();
    let key_id = public.legacy_key_id().to_string();
    let ascii_armor = public
        .to_armored_string(ArmorOptions::default())
        .expect("armor public key");

    let zip_bytes = build_zip_bytes();
    let zip_hash = sha256_hex(&zip_bytes);
    let shasums_body = format!("{zip_hash} *{ZIP_NAME}\n");

    let detached = DetachedSignature::sign_binary_data(
        &mut rng,
        &secret.primary_key,
        &Password::empty(),
        HashAlgorithm::Sha256,
        shasums_body.as_bytes(),
    )
    .expect("sign shasums");
    let sig_body = detached
        .to_armored_bytes(ArmorOptions::default())
        .expect("armor signature");

    let server = MockServer::start().await;
    let port = server.address().port();
    let base = format!("http://127.0.0.1:{port}");
    let reg_host = format!("127.0.0.1:{port}");

    Mock::given(method("GET"))
        .and(path("/.well-known/terraform.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "providers.v1": format!("{base}/v1/providers/")
        })))
        .mount(&server)
        .await;

    let pkg_path = format!("/v1/providers/{NS}/{PROVIDER}/{VERSION}/download/linux/amd64");
    let package_json = serde_json::json!({
        "protocols": ["5.0"],
        "os": "linux",
        "arch": "amd64",
        "filename": ZIP_NAME,
        "download_url": format!("{base}/release/{ZIP_NAME}"),
        "shasums_url": format!("{base}/release/SHA256SUMS"),
        "shasums_signature_url": format!("{base}/release/SHA256SUMS.sig"),
        "shasum": zip_hash,
        "signing_keys": {
            "gpg_public_keys": [{
                "key_id": key_id,
                "ascii_armor": ascii_armor,
            }]
        }
    });

    Mock::given(method("GET"))
        .and(path(&pkg_path))
        .respond_with(ResponseTemplate::new(200).set_body_json(package_json))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path(format!("/release/{ZIP_NAME}")))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(zip_bytes.clone()))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/release/SHA256SUMS"))
        .respond_with(ResponseTemplate::new(200).set_body_string(shasums_body.clone()))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/release/SHA256SUMS.sig"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(sig_body.clone()))
        .mount(&server)
        .await;

    let http = reqwest::Client::new();
    let registry = Registry::new(reg_host);
    let client = RegistryClient::with_http_client(registry, http);
    let provider_client = client.provider().await.expect("provider discovery");

    let dir = tempdir().expect("tempdir");
    let report = provider_client
        .download(
            NS,
            PROVIDER,
            VERSION,
            &[PLATFORM],
            dir.path().to_path_buf(),
        )
        .await
        .expect("download");

    let artifacts = report
        .get(PLATFORM)
        .expect("platform key")
        .as_ref()
        .expect("download ok");

    let platform_dir = dir.path().join(PLATFORM);
    let zip_path = platform_dir.join(ZIP_NAME);
    let shasums_path = platform_dir.join("terraform-provider-null_1.0.0_linux_amd64.shasums");
    let sig_path = platform_dir.join("terraform-provider-null_1.0.0_linux_amd64.shasums.sig");
    let keyring_path = platform_dir.join("terraform-provider-null_1.0.0_linux_amd64.shasums.asc");

    assert_eq!(std::fs::read(&zip_path).expect("read zip"), zip_bytes);
    assert_eq!(
        std::fs::read_to_string(&shasums_path).expect("read shasums"),
        shasums_body
    );
    assert_eq!(std::fs::read(&sig_path).expect("read sig"), sig_body);
    let keyring_on_disk = std::fs::read_to_string(&keyring_path).expect("read keyring");
    assert!(
        keyring_on_disk.contains("BEGIN PGP PUBLIC KEY BLOCK"),
        "keyring should contain exported public key"
    );

    let zip_artifact = artifacts
        .iter()
        .find(|a| a.filename == ZIP_NAME)
        .expect("zip artifact");
    assert_eq!(std::fs::read(&zip_artifact.path).expect("artifact path read"), zip_bytes);

    let bad = report.get("missing");
    assert!(bad.is_none());
}

#[tokio::test]
async fn download_propagates_mock_http_error_for_package_metadata() {
    let server = MockServer::start().await;
    let port = server.address().port();
    let base = format!("http://127.0.0.1:{port}");
    let reg_host = format!("127.0.0.1:{port}");

    Mock::given(method("GET"))
        .and(path("/.well-known/terraform.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "providers.v1": format!("{base}/v1/providers/")
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path(format!(
            "/v1/providers/{NS}/{PROVIDER}/{VERSION}/download/linux/amd64"
        )))
        .respond_with(ResponseTemplate::new(502))
        .mount(&server)
        .await;

    let http = reqwest::Client::new();
    let registry = Registry::new(reg_host);
    let client = RegistryClient::with_http_client(registry, http);
    let provider_client = client.provider().await.expect("provider");

    let dir = tempdir().expect("tempdir");
    let report = provider_client
        .download(
            NS,
            PROVIDER,
            VERSION,
            &[PLATFORM],
            dir.path().to_path_buf(),
        )
        .await
        .expect("outer result");

    let err = report
        .get(PLATFORM)
        .expect("entry")
        .as_ref()
        .expect_err("download error");
    assert!(matches!(
        err,
        PlatformDownloadError::Download(registry_client::ProviderRegistryError::UnsuccessfulStatus {
            status,
            ..
        }) if *status == reqwest::StatusCode::BAD_GATEWAY
    ));
}
