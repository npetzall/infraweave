//! Read path parity tests against legacy behavior fixtures.
//!
//! Validates mapping from DynamoDB item format (via fixture JSON) to catalog types.
//! See docs/BEHAVIOR_MATRIX.md and fixtures/README.md.

use catalog_aws::read;
use catalog_trait::read::ProjectionFields;
use serde_json::Value as JsonValue;
use std::path::Path;

fn fixture_path(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(name)
}

fn load_fixture_as_item(
    name: &str,
) -> std::collections::HashMap<String, aws_sdk_dynamodb::types::AttributeValue> {
    let path = fixture_path(name);
    let contents = std::fs::read_to_string(&path).expect("read fixture");
    let json: JsonValue = serde_json::from_str(&contents).expect("parse JSON");
    serde_dynamo::to_item(json).expect("convert to DynamoDB item")
}

#[test]
fn provider_fixture_maps_to_catalog_provider() {
    let item = load_fixture_as_item("provider_record.json");
    let resp = read::item_to_provider(&item).expect("deserialize ProviderResp");
    let provider = read::provider_resp_to_catalog(resp, None);

    assert_eq!(provider.reference.id, "providers/aws/005.045.000/aws.zip");
    assert_eq!(provider.metadata.as_ref().unwrap().name, "aws");
    assert_eq!(provider.metadata.as_ref().unwrap().version, "5.45.0");
    assert!(provider.manifest.is_some());
}

#[test]
fn module_fixture_maps_to_catalog_module() {
    let item = load_fixture_as_item("module_record.json");
    let resp = read::item_to_module(&item).expect("deserialize ModuleResp");
    let module = read::module_resp_to_module(&resp, None, None);

    assert_eq!(
        module.reference.id,
        "modules/stable/s3bucket/000.001.002/s3bucket.zip"
    );
    assert_eq!(module.metadata.as_ref().unwrap().name, "s3bucket");
    assert_eq!(module.metadata.as_ref().unwrap().track, "stable");
    assert_eq!(module.metadata.as_ref().unwrap().version, "0.1.2");
    assert!(module.manifest.is_some());
}

#[test]
fn stack_fixture_maps_to_catalog_stack() {
    let item = load_fixture_as_item("stack_record.json");
    let resp = read::item_to_module(&item).expect("deserialize ModuleResp");
    let stack = read::module_resp_to_stack(&resp, None, None);

    assert_eq!(
        stack.reference.id,
        "modules/stable/bucketcollection/000.002.000/bucketcollection.zip"
    );
    assert_eq!(stack.metadata.as_ref().unwrap().name, "bucketcollection");
    assert_eq!(stack.metadata.as_ref().unwrap().version, "0.2.0");
    assert!(stack.stack_data.is_some());
}

#[test]
fn projection_restricts_fields() {
    let item = load_fixture_as_item("module_record.json");
    let resp = read::item_to_module(&item).expect("deserialize");
    let module = read::module_resp_to_module(&resp, Some(ProjectionFields::METADATA), None);

    assert!(module.metadata.is_some());
    assert!(module.manifest.is_none());
    assert!(module.terraform.is_none());
    assert!(module.provider_mirror.is_none());
}

#[test]
fn pagination_token_roundtrip() {
    use std::collections::HashMap;
    let mut key = HashMap::new();
    key.insert(
        "PK".to_string(),
        aws_sdk_dynamodb::types::AttributeValue::S("LATEST_MODULE".to_string()),
    );
    key.insert(
        "SK".to_string(),
        aws_sdk_dynamodb::types::AttributeValue::S("MODULE#stable::s3bucket".to_string()),
    );

    let encoded = read::encode_next_token(&key).expect("encode");
    assert!(!encoded.is_empty());

    let decoded = read::decode_next_token(&encoded).expect("decode");
    assert_eq!(decoded.len(), 2);
    assert_eq!(
        decoded
            .get("PK")
            .and_then(|v| v.as_s().ok())
            .map(|s| s.as_str()),
        Some("LATEST_MODULE")
    );
}
