//! Internal serde models copied from `env_defs` (crate `defs`) for legacy API / DynamoDB parity.
//!
//! Source of truth for field layout and attributes: `defs/src/tfprovider.rs`, `defs/src/module.rs`,
//! `defs/src/tfoutput.rs`, `defs/src/oci.rs`. Keep in sync when evolving schemas.

use catalog_trait::read::ContentSource;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

// --- Source: defs/src/tfprovider.rs (struct `Metadata` in that file) ---
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct ProviderMetadata {
    pub name: String,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct ProviderManifest {
    pub metadata: ProviderMetadata,
    #[serde(rename = "apiVersion")]
    pub api_version: String,
    pub kind: String,
    pub spec: ProviderSpec,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ProviderSpec {
    pub provider: String,
    pub alias: Option<String>,
    pub version: Option<String>,
    pub description: String,
    pub reference: String,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct ProviderResp {
    pub name: String,
    pub version: String,
    pub timestamp: String,
    pub description: String,
    pub reference: String,
    pub manifest: ProviderManifest,
    #[serde(default)]
    pub tf_variables: Vec<TfVariable>,
    #[serde(default)]
    pub tf_extra_environment_variables: Vec<String>,
    pub s3_key: String,
    #[serde(default)]
    pub deprecated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deprecated_message: Option<String>,
    #[serde(default)]
    pub yanked: bool,
}

// --- Source: defs/src/tfoutput.rs ---
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct TfOutput {
    pub name: String,
    pub value: String,
    pub description: String,
    pub sensitive: Option<bool>,
}

// --- Source: defs/src/oci.rs (struct `OciArtifactSet`) ---
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct OciArtifactSet {
    pub oci_artifact_path: String,
    pub digest: String,
    #[serde(default)]
    pub tag_main: String,
    #[serde(default)]
    pub tag_signature: Option<String>,
    #[serde(default)]
    pub tag_attestation: Option<String>,
}

// --- Source: defs/src/module.rs (TfVariable through ModuleResp) ---

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct TfVariable {
    pub name: String,
    #[serde(rename = "type", default = "default_tf_variable_type")]
    pub _type: serde_json::Value,
    #[serde(
        default,
        deserialize_with = "deserialize_default_value_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub default: Option<serde_json::Value>,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_nullable")]
    pub nullable: bool,
    #[serde(default)]
    pub sensitive: bool,
}

fn default_tf_variable_type() -> serde_json::Value {
    serde_json::Value::String("any".to_string())
}

fn default_nullable() -> bool {
    true
}

fn deserialize_default_value_option<'de, D>(
    deserializer: D,
) -> Result<Option<serde_json::Value>, D::Error>
where
    D: Deserializer<'de>,
{
    let v = serde_json::Value::deserialize(deserializer)?;
    Ok(Some(v))
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct ModuleStackData {
    pub modules: Vec<StackModule>,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct StackModule {
    pub module: String,
    pub version: String,
    pub s3_key: String,
    #[serde(default)]
    pub track: String,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct TfRequiredProvider {
    pub name: String,
    pub version: String,
    pub source: String,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct TfLockProvider {
    pub source: String,
    pub version: String,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct ModuleDiffAddition {
    pub path: String,
    pub value: serde_json::Value,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct ModuleDiffRemoval {
    pub path: String,
    pub value: serde_json::Value,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct ModuleDiffChange {
    pub path: String,
    pub old_value: serde_json::Value,
    pub new_value: serde_json::Value,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct ModuleVersionDiff {
    pub added: Vec<ModuleDiffAddition>,
    pub changed: Vec<ModuleDiffChange>,
    pub removed: Vec<ModuleDiffRemoval>,
    pub previous_version: String,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default, PartialEq)]
pub struct ModuleResp {
    pub track: String,
    pub track_version: String,
    pub version: String,
    pub timestamp: String,
    #[serde(rename = "module_name")]
    pub module_name: String,
    pub module: String,
    pub module_type: String,
    pub description: String,
    pub reference: String,
    #[serde(deserialize_with = "deserialize_module_manifest")]
    pub manifest: ModuleManifest,
    pub tf_variables: Vec<TfVariable>,
    pub tf_outputs: Vec<TfOutput>,
    #[serde(default)]
    pub tf_providers: Vec<ProviderResp>,
    #[serde(default)]
    pub tf_required_providers: Vec<TfRequiredProvider>,
    #[serde(default)]
    pub tf_lock_providers: Vec<TfLockProvider>,
    #[serde(default)]
    pub tf_extra_environment_variables: Vec<String>,
    pub s3_key: String,
    pub oci_artifact_set: Option<OciArtifactSet>,
    pub stack_data: Option<ModuleStackData>,
    pub version_diff: Option<ModuleVersionDiff>,
    pub cpu: String,
    pub memory: String,
    #[serde(default)]
    pub deprecated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deprecated_message: Option<String>,
    #[serde(default)]
    pub yanked: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_mirror: Option<HashMap<PathBuf, ContentSource>>,
}

pub fn deserialize_module_manifest<'de, D>(deserializer: D) -> Result<ModuleManifest, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let val = Deserialize::deserialize(deserializer)?;
    let env = "aws";
    match env {
        "aws" => {
            if let serde_json::Value::Object(map) = val {
                serde_json::from_value(serde_json::Value::Object(map))
                    .map_err(serde::de::Error::custom)
            } else {
                Err(serde::de::Error::custom(
                    "Expected a JSON object for AWS manifest",
                ))
            }
        }
        "azure" => {
            if let serde_json::Value::String(str) = val {
                serde_json::from_str(&str).map_err(serde::de::Error::custom)
            } else {
                Err(serde::de::Error::custom(
                    "Expected a JSON string for Azure manifest",
                ))
            }
        }
        _ => Err(serde::de::Error::custom("Invalid ENV value")),
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, Default, PartialEq)]
pub struct ModuleManifest {
    pub metadata: Metadata,
    #[serde(rename = "apiVersion")]
    pub api_version: String,
    pub kind: String,
    pub spec: ModuleSpec,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct ModuleExample {
    pub name: String,
    pub description: String,
    pub variables: serde_yaml::Value,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default, PartialEq)]
pub struct ModuleSpec {
    #[serde(rename = "moduleName")]
    pub module_name: String,
    pub version: Option<String>,
    pub description: String,
    pub reference: String,
    pub examples: Option<Vec<ModuleExample>>,
    pub cpu: Option<String>,
    pub memory: Option<String>,
    #[serde(default)]
    pub providers: Vec<Provider>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct Metadata {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Provider {
    pub name: String,
}
