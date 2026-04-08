//! Read path implementation for DynamoDB/S3 catalog.
//!
//! Key patterns, mapping, and query builders per BEHAVIOR_MATRIX.md.

use aws_sdk_dynamodb::types::AttributeValue;
use base64::Engine;
use catalog_trait::read::{ContentSource, ProjectionFields};
use catalog_trait::types::{CatalogRef, Metadata, TerraformInterface};
use catalog_trait::{ModuleManifest, StackManifest, StackMetadata, StackSpec};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::compat::{compat_module_resp_into_catalog, compat_provider_resp_into_catalog};
use crate::compat_models::{ModuleResp, ProviderResp};
use crate::errors::CatalogError;

// --- Key helpers (canonical patterns from BEHAVIOR_MATRIX) ---

/// Module/stack identifier: `{track}::{name}`
pub fn get_module_identifier(name: &str, track: &str) -> String {
    format!("{}::{}", track, name)
}

/// Zero-pad semver for DynamoDB sort key (3 digits per component).
pub fn zero_pad_semver(ver_str: &str, pad_length: usize) -> Result<String, semver::Error> {
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

// --- Pagination token encoding ---

/// Encode LastEvaluatedKey as base64 JSON for next token.
pub fn encode_next_token(key: &HashMap<String, AttributeValue>) -> Option<String> {
    if key.is_empty() {
        return None;
    }
    let value: serde_json::Value = serde_dynamo::from_item(key.clone()).ok()?;
    let s = serde_json::to_string(&value).ok()?;
    Some(base64::engine::general_purpose::STANDARD.encode(s))
}

/// Decode next token to ExclusiveStartKey. Returns InvalidInput on decode failure.
pub fn decode_next_token(token: &str) -> Result<HashMap<String, AttributeValue>, CatalogError> {
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(token)
        .map_err(|e| CatalogError::InvalidInput {
            message: format!("invalid pagination token: {}", e),
            source: Some(anyhow::anyhow!("{}", e)),
        })?;
    let s = String::from_utf8(decoded).map_err(|e| CatalogError::InvalidInput {
        message: format!("invalid pagination token encoding: {}", e),
        source: Some(anyhow::anyhow!("{}", e)),
    })?;
    let json: serde_json::Value =
        serde_json::from_str(&s).map_err(|e| CatalogError::InvalidInput {
            message: format!("invalid pagination token format: {}", e),
            source: Some(anyhow::anyhow!("{}", e)),
        })?;
    let item = serde_dynamo::to_item(json).map_err(|e| CatalogError::InvalidInput {
        message: format!("invalid pagination token structure: {}", e),
        source: Some(anyhow::anyhow!("{}", e)),
    })?;
    Ok(item)
}

// --- Mapping: DynamoDB items -> catalog types ---

pub fn provider_resp_to_catalog(
    r: ProviderResp,
    projection: Option<ProjectionFields>,
) -> catalog_trait::read::Provider {
    let r = compat_provider_resp_into_catalog(r);
    let full = projection.is_none() || projection == Some(ProjectionFields::ALL);
    let proj = projection.unwrap_or(ProjectionFields::ALL);

    let metadata = if full || proj.contains(ProjectionFields::METADATA) {
        Some(Metadata {
            name: r.name.clone(),
            kind: "Provider".to_string(),
            track: String::new(),
            version: r.version.clone(),
            timestamp: r.timestamp.clone(),
            description: r.description.clone(),
            reference: r.reference.clone(),
            cpu: String::new(),
            memory: String::new(),
            deprecated: r.deprecated,
            deprecated_message: r.deprecated_message.clone(),
        })
    } else {
        None
    };

    let manifest = if full || proj.contains(ProjectionFields::MANIFEST) {
        Some(r.manifest)
    } else {
        None
    };

    let terraform = if full || proj.contains(ProjectionFields::TERRAFORM) {
        Some(TerraformInterface {
            tf_variables: r.tf_variables,
            tf_outputs: vec![],
            tf_providers: vec![],
            tf_required_providers: vec![],
            tf_lock_providers: vec![],
            tf_extra_environment_variables: r.tf_extra_environment_variables,
        })
    } else {
        None
    };

    catalog_trait::read::Provider {
        reference: CatalogRef {
            id: r.s3_key.clone(),
        },
        metadata,
        manifest,
        terraform,
    }
}

pub fn module_resp_to_module(
    r: &ModuleResp,
    projection: Option<ProjectionFields>,
    provider_mirror_override: Option<HashMap<PathBuf, ContentSource>>,
) -> catalog_trait::read::Module {
    let stored_provider_mirror = r.provider_mirror.clone();
    let r = compat_module_resp_into_catalog(r.clone());
    let full = projection.is_none() || projection == Some(ProjectionFields::ALL);
    let proj = projection.unwrap_or(ProjectionFields::ALL);

    let metadata = if full || proj.contains(ProjectionFields::METADATA) {
        Some(Metadata {
            name: r.module.clone(),
            kind: r.module_type.clone(),
            track: r.track.clone(),
            version: r.version.clone(),
            timestamp: r.timestamp.clone(),
            description: r.description.clone(),
            reference: r.reference.clone(),
            cpu: r.cpu.clone(),
            memory: r.memory.clone(),
            deprecated: r.deprecated,
            deprecated_message: r.deprecated_message.clone(),
        })
    } else {
        None
    };

    let manifest = if full || proj.contains(ProjectionFields::MANIFEST) {
        Some(r.manifest.clone())
    } else {
        None
    };

    let terraform = if full || proj.contains(ProjectionFields::TERRAFORM) {
        Some(TerraformInterface {
            tf_variables: r.tf_variables.clone(),
            tf_outputs: r.tf_outputs.clone(),
            tf_providers: r.tf_providers.clone(),
            tf_required_providers: r.tf_required_providers.clone(),
            tf_lock_providers: r.tf_lock_providers.clone(),
            tf_extra_environment_variables: r.tf_extra_environment_variables.clone(),
        })
    } else {
        None
    };

    let provider_mirror = if full || proj.contains(ProjectionFields::PROVIDER_MIRROR) {
        provider_mirror_override
            .or(stored_provider_mirror)
            .filter(|m| !m.is_empty())
    } else {
        None
    };

    catalog_trait::read::Module {
        reference: CatalogRef {
            id: r.s3_key.clone(),
        },
        metadata,
        manifest,
        terraform,
        provider_mirror,
    }
}

pub fn module_resp_to_stack(
    r: &ModuleResp,
    projection: Option<ProjectionFields>,
    provider_mirror_override: Option<HashMap<PathBuf, ContentSource>>,
) -> catalog_trait::read::Stack {
    let stored_provider_mirror = r.provider_mirror.clone();
    let r = compat_module_resp_into_catalog(r.clone());
    let full = projection.is_none() || projection == Some(ProjectionFields::ALL);
    let proj = projection.unwrap_or(ProjectionFields::ALL);

    let metadata = if full || proj.contains(ProjectionFields::METADATA) {
        Some(Metadata {
            name: r.module.clone(),
            kind: r.module_type.clone(),
            track: r.track.clone(),
            version: r.version.clone(),
            timestamp: r.timestamp.clone(),
            description: r.description.clone(),
            reference: r.reference.clone(),
            cpu: r.cpu.clone(),
            memory: r.memory.clone(),
            deprecated: r.deprecated,
            deprecated_message: r.deprecated_message.clone(),
        })
    } else {
        None
    };

    let manifest = if full || proj.contains(ProjectionFields::MANIFEST) {
        Some(module_manifest_to_stack_manifest(&r.manifest))
    } else {
        None
    };

    let terraform = if full || proj.contains(ProjectionFields::TERRAFORM) {
        Some(TerraformInterface {
            tf_variables: r.tf_variables.clone(),
            tf_outputs: r.tf_outputs.clone(),
            tf_providers: r.tf_providers.clone(),
            tf_required_providers: r.tf_required_providers.clone(),
            tf_lock_providers: r.tf_lock_providers.clone(),
            tf_extra_environment_variables: r.tf_extra_environment_variables.clone(),
        })
    } else {
        None
    };

    let stack_data = if full || proj.contains(ProjectionFields::STACK_DATA) {
        r.stack_data.clone()
    } else {
        None
    };

    let provider_mirror = if full || proj.contains(ProjectionFields::PROVIDER_MIRROR) {
        provider_mirror_override
            .or(stored_provider_mirror)
            .filter(|m| !m.is_empty())
    } else {
        None
    };

    catalog_trait::read::Stack {
        reference: CatalogRef {
            id: r.s3_key.clone(),
        },
        metadata,
        manifest,
        terraform,
        stack_data,
        provider_mirror,
    }
}

/// Deserialize DynamoDB item to ProviderResp.
pub fn item_to_provider(
    item: &HashMap<String, AttributeValue>,
) -> Result<ProviderResp, CatalogError> {
    serde_dynamo::from_item(item.clone()).map_err(|e| CatalogError::Serialization {
        context: "ProviderResp".to_string(),
        source: anyhow::anyhow!("{}", e),
    })
}

/// Deserialize DynamoDB item to ModuleResp.
pub fn item_to_module(item: &HashMap<String, AttributeValue>) -> Result<ModuleResp, CatalogError> {
    serde_dynamo::from_item(item.clone()).map_err(|e| CatalogError::Serialization {
        context: "ModuleResp".to_string(),
        source: anyhow::anyhow!("{}", e),
    })
}

fn module_manifest_to_stack_manifest(m: &ModuleManifest) -> StackManifest {
    StackManifest {
        metadata: StackMetadata {
            name: m.metadata.name.clone(),
        },
        api_version: m.api_version.clone(),
        kind: m.kind.clone(),
        spec: StackSpec {
            stack_name: m.spec.module_name.clone(),
            version: m.spec.version.clone(),
            description: m.spec.description.clone(),
            reference: m.spec.reference.clone(),
            examples: m.spec.examples.clone(),
            cpu: m.spec.cpu.clone(),
            memory: m.spec.memory.clone(),
            locals: None,
            dependencies: None,
            stack_variable_definitions: None,
        },
    }
}
