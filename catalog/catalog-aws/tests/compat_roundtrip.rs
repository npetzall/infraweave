//! Legacy payload roundtrip: DynamoDB item → legacy models → catalog → legacy models (structs must match).
//!
//! JSON byte equality is not required: serde omits optional fields (`deprecated_message`, `yanked`)
//! depending on serialization; struct equality is the parity contract.

use catalog_aws::compat::{
    catalog_module_to_legacy, catalog_provider_to_legacy, catalog_stack_to_legacy,
};
use catalog_aws::compat_models::ModuleResp;
use catalog_aws::read;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::path::Path;

fn fixture_path(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(name)
}

fn load_fixture_json(name: &str) -> JsonValue {
    let path = fixture_path(name);
    let contents = std::fs::read_to_string(&path).expect("read fixture");
    serde_json::from_str(&contents).expect("parse JSON")
}

fn load_fixture_as_item(name: &str) -> HashMap<String, aws_sdk_dynamodb::types::AttributeValue> {
    let json = load_fixture_json(name);
    serde_dynamo::to_item(json).expect("convert to DynamoDB item")
}

#[test]
fn provider_fixture_legacy_roundtrip() {
    let item = load_fixture_as_item("provider_record.json");
    let resp = read::item_to_provider(&item).expect("deserialize");
    let provider = read::provider_resp_to_catalog(resp.clone(), None);
    let legacy = catalog_provider_to_legacy(&provider);
    assert_eq!(resp, legacy);
}

#[test]
fn module_fixture_legacy_roundtrip() {
    let item = load_fixture_as_item("module_record.json");
    let resp = read::item_to_module(&item).expect("deserialize");
    let module = read::module_resp_to_module(&resp, None, None);
    let legacy = catalog_module_to_legacy(&module);
    assert_eq!(resp, legacy);
}

#[test]
fn stack_fixture_legacy_roundtrip() {
    let item = load_fixture_as_item("stack_record.json");
    let resp = read::item_to_module(&item).expect("deserialize");
    let stack = read::module_resp_to_stack(&resp, None, None);
    let legacy = catalog_stack_to_legacy(&stack);
    assert_resp_matches_stack_roundtrip(&resp, &legacy);
}

/// Stack records in DynamoDB use `ModuleResp`; the manifest JSON may include stack-only spec keys
/// that `ModuleSpec` does not retain. Compare fields that survive the read path and legacy adapter.
fn assert_resp_matches_stack_roundtrip(a: &ModuleResp, b: &ModuleResp) {
    assert_eq!(a.track, b.track);
    assert_eq!(a.track_version, b.track_version);
    assert_eq!(a.version, b.version);
    assert_eq!(a.timestamp, b.timestamp);
    assert_eq!(a.module_name, b.module_name);
    assert_eq!(a.module, b.module);
    assert_eq!(a.module_type, b.module_type);
    assert_eq!(a.description, b.description);
    assert_eq!(a.reference, b.reference);
    assert_eq!(a.s3_key, b.s3_key);
    assert_eq!(a.tf_variables, b.tf_variables);
    assert_eq!(a.tf_outputs, b.tf_outputs);
    assert_eq!(a.tf_providers, b.tf_providers);
    assert_eq!(a.tf_required_providers, b.tf_required_providers);
    assert_eq!(a.tf_lock_providers, b.tf_lock_providers);
    assert_eq!(
        a.tf_extra_environment_variables,
        b.tf_extra_environment_variables
    );
    assert_eq!(a.oci_artifact_set, b.oci_artifact_set);
    assert_eq!(a.stack_data, b.stack_data);
    assert_eq!(a.version_diff, b.version_diff);
    assert_eq!(a.cpu, b.cpu);
    assert_eq!(a.memory, b.memory);
    assert_eq!(a.deprecated, b.deprecated);
    assert_eq!(a.deprecated_message, b.deprecated_message);
    assert_eq!(a.yanked, b.yanked);
    assert_eq!(a.manifest, b.manifest);
}
