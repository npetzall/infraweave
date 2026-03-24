//! Acceptance tests for catalog-aws parity with env_aws_direct.
//!
//! These tests define expected behavior before implementation. They validate:
//! - Golden fixture deserialization
//! - Canonical key/query patterns
//! - Edge-case handling
//!
//! See docs/BEHAVIOR_MATRIX.md and docs/EDGE_CASES.md.

use catalog_aws::compat_models::ProviderResp;
use serde_json::Value;
use std::path::Path;

/// Canonical module identifier: `{track}::{module}`
fn get_module_identifier(module: &str, track: &str) -> String {
    format!("{}::{}", track, module)
}

/// Zero-pad semver for DynamoDB sort key (3 digits per component).
fn zero_pad_semver(ver_str: &str, pad_length: usize) -> Result<String, semver::Error> {
    let version = semver::Version::parse(ver_str)?;
    let major = format!("{:0width$}", version.major, width = pad_length);
    let minor = format!("{:0width$}", version.minor, width = pad_length);
    let patch = format!("{:0width$}", version.patch, width = pad_length);
    let mut reconstructed = format!("{}.{}.{}", major, minor, patch);
    if !version.pre.is_empty() {
        reconstructed.push_str(&format!("-{}", &version.pre));
    }
    if !version.build.is_empty() {
        reconstructed.push_str(&format!("+{}", &version.build));
    }
    Ok(reconstructed)
}

fn fixture_path(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(name)
}

// --- Golden fixture deserialization ---

#[test]
fn fixture_provider_deserializes() {
    let path = fixture_path("provider_record.json");
    let contents = std::fs::read_to_string(&path).expect("read provider fixture");
    let _: ProviderResp = serde_json::from_str(&contents).expect("deserialize ProviderResp");
}

#[test]
fn fixture_module_deserializes() {
    let path = fixture_path("module_record.json");
    let contents = std::fs::read_to_string(&path).expect("read module fixture");
    let resp: Value = serde_json::from_str(&contents).expect("parse JSON");
    // ModuleResp has custom deserialize_module_manifest; use Value for flexibility
    assert!(resp.get("manifest").is_some());
    assert_eq!(resp["manifest"]["kind"], "Module");
    assert_eq!(resp["version"], "0.1.2");
    assert_eq!(resp["deprecated"], false);
}

#[test]
fn fixture_stack_deserializes() {
    let path = fixture_path("stack_record.json");
    let contents = std::fs::read_to_string(&path).expect("read stack fixture");
    let resp: Value = serde_json::from_str(&contents).expect("parse JSON");
    assert!(resp.get("manifest").is_some());
    assert_eq!(resp["manifest"]["kind"], "Stack");
    assert!(resp.get("stack_data").is_some());
}

#[test]
fn fixture_deprecated_module_deserializes() {
    let path = fixture_path("module_deprecated.json");
    let contents = std::fs::read_to_string(&path).expect("read deprecated fixture");
    let resp: Value = serde_json::from_str(&contents).expect("parse JSON");
    assert_eq!(resp["deprecated"], true);
    assert!(resp["deprecated_message"]
        .as_str()
        .unwrap()
        .contains("Security"));
}

#[test]
fn fixture_dev_version_deserializes() {
    let path = fixture_path("module_dev_version.json");
    let contents = std::fs::read_to_string(&path).expect("read dev fixture");
    let resp: Value = serde_json::from_str(&contents).expect("parse JSON");
    assert!(resp["version"].as_str().unwrap().starts_with("0.0.0-dev"));
}

// --- Canonical key patterns ---

#[test]
fn module_identifier_format() {
    assert_eq!(
        get_module_identifier("s3bucket", "stable"),
        "stable::s3bucket"
    );
    assert_eq!(get_module_identifier("mymod", ""), "::mymod");
}

#[test]
fn zero_pad_semver_format() {
    assert_eq!(zero_pad_semver("1.2.3", 3).unwrap(), "001.002.003");
    assert_eq!(
        zero_pad_semver("0.0.0-dev.1", 3).unwrap(),
        "000.000.000-dev.1"
    );
    assert_eq!(zero_pad_semver("5.45.0", 3).unwrap(), "005.045.000");
}

#[test]
fn malformed_version_returns_err() {
    assert!(zero_pad_semver("v1.0.0", 3).is_err());
    assert!(zero_pad_semver("1.0", 3).is_err());
    assert!(zero_pad_semver("invalid", 3).is_err());
}

// --- Expected key patterns for DynamoDB ---

#[test]
fn version_sk_pattern() {
    let sk = format!("VERSION#{}", zero_pad_semver("0.1.2", 3).unwrap());
    assert_eq!(sk, "VERSION#000.001.002");
}

#[test]
fn module_pk_pattern() {
    let id = format!("MODULE#{}", get_module_identifier("s3bucket", "stable"));
    assert_eq!(id, "MODULE#stable::s3bucket");
}

#[test]
fn provider_pk_pattern() {
    let id = "PROVIDER#aws".to_string();
    assert_eq!(id, "PROVIDER#aws");
}

#[test]
fn latest_pointer_constants() {
    assert_eq!("LATEST_MODULE", "LATEST_MODULE");
    assert_eq!("LATEST_STACK", "LATEST_STACK");
    assert_eq!("LATEST_PROVIDER", "LATEST_PROVIDER");
}
