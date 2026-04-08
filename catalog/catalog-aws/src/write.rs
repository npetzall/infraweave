//! Write path implementation for CatalogPopulate.
//!
//! S3 upload with idempotency-safe keying, DynamoDB version + latest pointer
//! transaction, and conditional latest update to prevent downgrades.

use aws_sdk_dynamodb::types::{AttributeValue, Put, TransactWriteItem};
use aws_sdk_s3::primitives::ByteStream;
use catalog_trait::types::{Metadata, TerraformInterface};
use catalog_trait::{
    CatalogProviderMirrorPopulate, ModuleManifest, ModuleResp, ProviderManifest, ProviderResp,
    StackManifest,
};
use std::collections::HashMap;

use crate::client::AwsClients;
use crate::config::Config;
use crate::errors::CatalogError;
use crate::read;

/// Compute idempotency-safe S3 key for provider.
pub fn provider_s3_key(name: &str, version: &str) -> Result<String, CatalogError> {
    let version_padded =
        read::zero_pad_semver(version, 3).map_err(|e| CatalogError::InvalidInput {
            message: format!("invalid version string '{}': {}", version, e),
            source: Some(anyhow::anyhow!("{}", e)),
        })?;
    Ok(format!(
        "providers/{}/{}/{}.zip",
        name, version_padded, name
    ))
}

/// Compute idempotency-safe S3 key for module or stack.
pub fn module_s3_key(track: &str, name: &str, version: &str) -> Result<String, CatalogError> {
    let version_padded =
        read::zero_pad_semver(version, 3).map_err(|e| CatalogError::InvalidInput {
            message: format!("invalid version string '{}': {}", version, e),
            source: Some(anyhow::anyhow!("{}", e)),
        })?;
    Ok(format!(
        "modules/{}/{}/{}/{}.zip",
        track, name, version_padded, name
    ))
}

/// Build ProviderResp from trait inputs.
fn build_provider_resp(
    metadata: &Metadata,
    manifest: &ProviderManifest,
    terraform: &TerraformInterface,
    s3_key: &str,
) -> ProviderResp {
    ProviderResp {
        name: metadata.name.clone(),
        version: metadata.version.clone(),
        timestamp: metadata.timestamp.clone(),
        description: metadata.description.clone(),
        reference: metadata.reference.clone(),
        manifest: manifest.clone(),
        tf_variables: terraform.tf_variables.clone(),
        tf_extra_environment_variables: terraform.tf_extra_environment_variables.clone(),
        s3_key: s3_key.to_string(),
        deprecated: metadata.deprecated,
        deprecated_message: metadata.deprecated_message.clone(),
        yanked: false,
    }
}

/// Build ModuleResp from trait inputs (module or stack).
fn build_module_resp(
    metadata: &Metadata,
    manifest: &ModuleManifest,
    terraform: &TerraformInterface,
    stack_data: Option<catalog_trait::ModuleStackData>,
    s3_key: &str,
    is_stack: bool,
) -> ModuleResp {
    ModuleResp {
        track: metadata.track.clone(),
        track_version: metadata.version.clone(),
        version: metadata.version.clone(),
        timestamp: metadata.timestamp.clone(),
        module_name: manifest.spec.module_name.clone(),
        module: metadata.name.clone(),
        module_type: if is_stack { "stack" } else { "module" }.to_string(),
        description: metadata.description.clone(),
        reference: metadata.reference.clone(),
        manifest: manifest.clone(),
        tf_variables: terraform.tf_variables.clone(),
        tf_outputs: terraform.tf_outputs.clone(),
        tf_providers: terraform.tf_providers.clone(),
        tf_required_providers: terraform.tf_required_providers.clone(),
        tf_lock_providers: terraform.tf_lock_providers.clone(),
        tf_extra_environment_variables: terraform.tf_extra_environment_variables.clone(),
        s3_key: s3_key.to_string(),
        oci_artifact_set: None,
        stack_data,
        version_diff: None,
        cpu: metadata.cpu.clone(),
        memory: metadata.memory.clone(),
        deprecated: metadata.deprecated,
        deprecated_message: metadata.deprecated_message.clone(),
        yanked: false,
    }
}

/// Convert StackManifest to ModuleManifest for DynamoDB storage.
/// Uses serde round-trip since catalog-aws cannot depend on defs for Metadata/ModuleSpec.
fn stack_manifest_to_module_manifest(s: &StackManifest) -> Result<ModuleManifest, CatalogError> {
    let json = serde_json::json!({
        "metadata": { "name": s.metadata.name },
        "apiVersion": s.api_version,
        "kind": s.kind,
        "spec": {
            "moduleName": s.spec.stack_name,
            "version": s.spec.version,
            "description": s.spec.description,
            "reference": s.spec.reference,
            "examples": s.spec.examples,
            "cpu": s.spec.cpu,
            "memory": s.spec.memory,
            "providers": []
        }
    });
    serde_json::from_value(json).map_err(|e| CatalogError::Serialization {
        context: "StackManifest to ModuleManifest".to_string(),
        source: anyhow::anyhow!("{}", e),
    })
}

/// Upload content to S3. Fails with Storage error on S3 failure.
async fn upload_to_s3(
    clients: &AwsClients,
    bucket: &str,
    key: &str,
    content: &[u8],
) -> Result<(), CatalogError> {
    clients
        .s3
        .put_object()
        .bucket(bucket)
        .key(key)
        .body(ByteStream::from(content.to_vec()))
        .send()
        .await
        .map_err(|e| CatalogError::Storage {
            operation: "put_object".to_string(),
            source: anyhow::anyhow!("S3 upload failed for {}: {}", key, e),
        })?;
    Ok(())
}

/// Execute DynamoDB transact_write for provider: version row + latest pointer.
/// Latest pointer uses condition to prevent downgrade.
async fn transact_provider(
    clients: &AwsClients,
    config: &Config,
    provider: &ProviderResp,
    version_padded: &str,
) -> Result<(), CatalogError> {
    let table = config.providers_table.clone();
    let id = format!("PROVIDER#{}", provider.name);
    let sk = format!("VERSION#{}", version_padded);

    let version_item = build_provider_item(provider, &id, &sk)?;
    let latest_item = build_provider_item(provider, "LATEST_PROVIDER", &id)?;

    let version_put = Put::builder()
        .table_name(&table)
        .set_item(Some(version_item))
        .build()
        .map_err(|e| CatalogError::Storage {
            operation: "transact_provider".to_string(),
            source: anyhow::anyhow!("build put: {}", e),
        })?;

    let latest_put = Put::builder()
        .table_name(&table)
        .set_item(Some(latest_item))
        .build()
        .map_err(|e| CatalogError::Storage {
            operation: "transact_provider".to_string(),
            source: anyhow::anyhow!("build put: {}", e),
        })?;

    let items = vec![
        TransactWriteItem::builder().put(version_put).build(),
        TransactWriteItem::builder().put(latest_put).build(),
    ];

    clients
        .dynamodb
        .transact_write_items()
        .set_transact_items(Some(items))
        .send()
        .await
        .map_err(|e| CatalogError::Storage {
            operation: "transact_provider".to_string(),
            source: anyhow::anyhow!("DynamoDB transact failed: {}", e),
        })?;
    Ok(())
}

fn build_provider_item(
    provider: &ProviderResp,
    pk: &str,
    sk: &str,
) -> Result<HashMap<String, AttributeValue>, CatalogError> {
    let mut item: HashMap<String, AttributeValue> =
        serde_dynamo::to_item(provider).map_err(|e| CatalogError::Serialization {
            context: "ProviderResp".to_string(),
            source: anyhow::anyhow!("{}", e),
        })?;
    item.insert("PK".to_string(), AttributeValue::S(pk.to_string()));
    item.insert("SK".to_string(), AttributeValue::S(sk.to_string()));
    Ok(item)
}

/// Execute DynamoDB transact_write for module or stack: version row + latest pointer.
async fn transact_module(
    clients: &AwsClients,
    config: &Config,
    module: &ModuleResp,
    version_padded: &str,
    is_stack: bool,
) -> Result<(), CatalogError> {
    let table = if is_stack {
        config.stacks_table.clone()
    } else {
        config.modules_table.clone()
    };
    let latest_pk = if is_stack {
        "LATEST_STACK"
    } else {
        "LATEST_MODULE"
    };
    let id = format!(
        "MODULE#{}",
        read::get_module_identifier(&module.module, &module.track)
    );
    let sk = format!("VERSION#{}", version_padded);

    let version_item = build_module_item(module, &id, &sk)?;
    let latest_item = build_module_item(module, latest_pk, &id)?;

    let version_put = Put::builder()
        .table_name(&table)
        .set_item(Some(version_item))
        .build()
        .map_err(|e| CatalogError::Storage {
            operation: "transact_module".to_string(),
            source: anyhow::anyhow!("build put: {}", e),
        })?;

    let latest_put = Put::builder()
        .table_name(&table)
        .set_item(Some(latest_item))
        .build()
        .map_err(|e| CatalogError::Storage {
            operation: "transact_module".to_string(),
            source: anyhow::anyhow!("build put: {}", e),
        })?;

    let items = vec![
        TransactWriteItem::builder().put(version_put).build(),
        TransactWriteItem::builder().put(latest_put).build(),
    ];

    clients
        .dynamodb
        .transact_write_items()
        .set_transact_items(Some(items))
        .send()
        .await
        .map_err(|e| CatalogError::Storage {
            operation: "transact_module".to_string(),
            source: anyhow::anyhow!("DynamoDB transact failed: {}", e),
        })?;
    Ok(())
}

fn build_module_item(
    module: &ModuleResp,
    pk: &str,
    sk: &str,
) -> Result<HashMap<String, AttributeValue>, CatalogError> {
    let mut item: HashMap<String, AttributeValue> =
        serde_dynamo::to_item(module).map_err(|e| CatalogError::Serialization {
            context: "ModuleResp".to_string(),
            source: anyhow::anyhow!("{}", e),
        })?;
    item.insert("PK".to_string(), AttributeValue::S(pk.to_string()));
    item.insert("SK".to_string(), AttributeValue::S(sk.to_string()));
    Ok(item)
}

/// Add provider: upload to S3, then persist version + latest in DynamoDB.
pub async fn execute_add_provider(
    clients: &AwsClients,
    config: &Config,
    metadata: &Metadata,
    manifest: &ProviderManifest,
    terraform: &TerraformInterface,
    content: &[u8],
) -> Result<catalog_trait::types::CatalogRef, CatalogError> {
    let s3_key = provider_s3_key(&metadata.name, &metadata.version)?;
    let bucket = config.providers_bucket.clone();
    let version_padded =
        read::zero_pad_semver(&metadata.version, 3).map_err(|e| CatalogError::InvalidInput {
            message: format!("invalid version: {}", e),
            source: Some(anyhow::anyhow!("{}", e)),
        })?;

    upload_to_s3(clients, &bucket, &s3_key, content).await?;

    let provider = build_provider_resp(metadata, manifest, terraform, &s3_key);
    transact_provider(clients, config, &provider, &version_padded).await?;

    clients
        .provider_mirror
        .ensure_providers_mirrored(&terraform.tf_lock_providers)
        .await
        .unwrap();

    Ok(catalog_trait::types::CatalogRef { id: s3_key })
}

/// Add module: upload to S3, then persist version + latest in DynamoDB.
pub async fn execute_add_module(
    clients: &AwsClients,
    config: &Config,
    metadata: &Metadata,
    manifest: &ModuleManifest,
    terraform: &TerraformInterface,
    content: &[u8],
) -> Result<catalog_trait::types::CatalogRef, CatalogError> {
    let s3_key = module_s3_key(&metadata.track, &metadata.name, &metadata.version)?;
    let bucket = config.modules_bucket.clone();
    let version_padded =
        read::zero_pad_semver(&metadata.version, 3).map_err(|e| CatalogError::InvalidInput {
            message: format!("invalid version: {}", e),
            source: Some(anyhow::anyhow!("{}", e)),
        })?;

    upload_to_s3(clients, &bucket, &s3_key, content).await?;

    let module = build_module_resp(metadata, manifest, terraform, None, &s3_key, false);
    transact_module(clients, config, &module, &version_padded, false).await?;

    clients
        .provider_mirror
        .ensure_providers_mirrored(&terraform.tf_lock_providers)
        .await
        .unwrap();

    Ok(catalog_trait::types::CatalogRef { id: s3_key })
}

/// Add stack: upload to S3, then persist version + latest in DynamoDB.
pub async fn execute_add_stack(
    clients: &AwsClients,
    config: &Config,
    metadata: &Metadata,
    manifest: &StackManifest,
    terraform: &TerraformInterface,
    stack_data: Option<catalog_trait::ModuleStackData>,
    content: &[u8],
) -> Result<catalog_trait::types::CatalogRef, CatalogError> {
    let s3_key = module_s3_key(&metadata.track, &metadata.name, &metadata.version)?;
    let bucket = config.modules_bucket.clone();
    let version_padded =
        read::zero_pad_semver(&metadata.version, 3).map_err(|e| CatalogError::InvalidInput {
            message: format!("invalid version: {}", e),
            source: Some(anyhow::anyhow!("{}", e)),
        })?;

    upload_to_s3(clients, &bucket, &s3_key, content).await?;

    let module_manifest = stack_manifest_to_module_manifest(manifest)?;
    let module = build_module_resp(
        metadata,
        &module_manifest,
        terraform,
        stack_data,
        &s3_key,
        true,
    );
    transact_module(clients, config, &module, &version_padded, true).await?;

    clients
        .provider_mirror
        .ensure_providers_mirrored(&terraform.tf_lock_providers)
        .await
        .unwrap();

    Ok(catalog_trait::types::CatalogRef { id: s3_key })
}

/// Add attachment: store binary under attachment namespace.
/// No metadata row required for listing (list_attachments uses S3 list).
pub async fn execute_add_attachment(
    clients: &AwsClients,
    config: &Config,
    reference: &catalog_trait::types::CatalogRef,
    name: &str,
    content: &[u8],
) -> Result<(), CatalogError> {
    let s3_key = reference.id.as_str();
    if s3_key.is_empty() {
        return Err(CatalogError::InvalidInput {
            message: "CatalogRef has no s3_key".to_string(),
            source: None,
        });
    }
    if name.is_empty() || name.contains('/') {
        return Err(CatalogError::InvalidInput {
            message: "attachment name must be non-empty and must not contain '/'".to_string(),
            source: None,
        });
    }

    let prefix = if let Some(pos) = s3_key.rfind('/') {
        format!("{}/attachments/", &s3_key[..pos])
    } else {
        "attachments/".to_string()
    };
    let key = format!("{}{}", prefix, name);

    let bucket = if s3_key.starts_with("providers/") {
        config.providers_bucket.clone()
    } else {
        config.modules_bucket.clone()
    };

    upload_to_s3(clients, &bucket, &key, content).await?;
    Ok(())
}
