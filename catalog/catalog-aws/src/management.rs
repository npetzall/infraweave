//! Management operations: promote, deprecate, yank.
//!
//! Implements CatalogManagement with conditional writes and audit metadata.

use aws_sdk_dynamodb::types::AttributeValue;
use catalog_trait::types::{CatalogKind, CatalogRef};

use crate::client::AwsClients;
use crate::config::Config;
use crate::errors::CatalogError;
use crate::ops;
use crate::read;

/// Parsed components from a catalog reference s3_key.
#[allow(dead_code)]
#[derive(Debug)]
struct ParsedRef {
    name: String,
    track: String,
    version: String,
    version_padded: String,
}

/// Convert zero-padded version back to semantic version for VersionSelector.
fn unpad_version(padded: &str) -> String {
    let (base, rest) = padded.split_once('-').unwrap_or((padded, ""));
    let parts: Vec<&str> = base.split('.').collect();
    let major = parts
        .get(0)
        .and_then(|p| p.parse::<u64>().ok())
        .unwrap_or(0);
    let minor = parts
        .get(1)
        .and_then(|p| p.parse::<u64>().ok())
        .unwrap_or(0);
    let patch = parts
        .get(2)
        .and_then(|p| p.split('-').next())
        .and_then(|p| p.parse::<u64>().ok())
        .unwrap_or(0);
    let mut s = format!("{}.{}.{}", major, minor, patch);
    if !rest.is_empty() {
        s.push_str("-");
        s.push_str(rest);
    }
    s
}

/// Parse provider s3_key: providers/{name}/{version_padded}/{name}.zip
fn parse_provider_s3_key(s3_key: &str) -> Result<ParsedRef, CatalogError> {
    let parts: Vec<&str> = s3_key.split('/').collect();
    if parts.len() < 4 || parts[0] != "providers" {
        return Err(CatalogError::InvalidInput {
            message: format!("invalid provider s3_key: expected providers/{{name}}/{{version}}/{{name}}.zip, got {}", s3_key),
            source: None,
        });
    }
    let name = parts[1].to_string();
    let version_padded = parts[2].to_string();
    let version = unpad_version(&version_padded);
    Ok(ParsedRef {
        name: name.clone(),
        track: String::new(),
        version,
        version_padded,
    })
}

/// Parse module/stack s3_key: modules/{track}/{name}/{version_padded}/{name}.zip
fn parse_module_s3_key(s3_key: &str) -> Result<ParsedRef, CatalogError> {
    let parts: Vec<&str> = s3_key.split('/').collect();
    if parts.len() < 5 || parts[0] != "modules" {
        return Err(CatalogError::InvalidInput {
            message: format!(
                "invalid module/stack s3_key: expected modules/{{track}}/{{name}}/{{version}}/{{name}}.zip, got {}",
                s3_key
            ),
            source: None,
        });
    }
    let track = parts[1].to_string();
    let name = parts[2].to_string();
    let version_padded = parts[3].to_string();
    let version = unpad_version(&version_padded);
    Ok(ParsedRef {
        name: name.clone(),
        track,
        version,
        version_padded,
    })
}

fn parse_s3_key(kind: CatalogKind, s3_key: &str) -> Result<ParsedRef, CatalogError> {
    if s3_key.is_empty() {
        return Err(CatalogError::InvalidInput {
            message: "CatalogRef has no s3_key".to_string(),
            source: None,
        });
    }
    match kind {
        CatalogKind::Provider => parse_provider_s3_key(s3_key),
        CatalogKind::Module | CatalogKind::Stack => parse_module_s3_key(s3_key),
    }
}

/// Promote: re-point target track/version pointer to an existing version record.
pub async fn execute_promote(
    clients: &AwsClients,
    config: &Config,
    kind: CatalogKind,
    reference: &CatalogRef,
    track: &str,
    version: Option<&str>,
) -> Result<(), CatalogError> {
    let parsed = parse_s3_key(kind, &reference.id)?;
    let version_to_use = version.unwrap_or(&parsed.version);
    let _version_padded =
        read::zero_pad_semver(version_to_use, 3).map_err(|e| CatalogError::InvalidInput {
            message: format!("invalid version '{}': {}", version_to_use, e),
            source: Some(anyhow::anyhow!("{}", e)),
        })?;

    let (table, latest_pk, target_latest_sk) = match kind {
        CatalogKind::Provider => {
            let latest_sk = format!("PROVIDER#{}", parsed.name);
            (config.providers_table.clone(), "LATEST_PROVIDER", latest_sk)
        }
        CatalogKind::Module => {
            let target_id = read::get_module_identifier(&parsed.name, track);
            (
                config.modules_table.clone(),
                "LATEST_MODULE",
                format!("MODULE#{}", target_id),
            )
        }
        CatalogKind::Stack => {
            let target_id = read::get_module_identifier(&parsed.name, track);
            (
                config.stacks_table.clone(),
                "LATEST_STACK",
                format!("MODULE#{}", target_id),
            )
        }
    };

    // Fetch the version record to validate it exists
    let version_selector = catalog_trait::types::VersionSelector::Exact(version_to_use.to_string());
    let track_for_get = match kind {
        CatalogKind::Provider => "",
        CatalogKind::Module | CatalogKind::Stack => &parsed.track,
    };
    let version_item = ops::execute_get(
        clients,
        config,
        kind,
        &parsed.name,
        track_for_get,
        &version_selector,
    )
    .await?
    .ok_or_else(|| CatalogError::NotFound {
        kind: format!("{:?}", kind),
        key: format!("{}@{}#{}", parsed.name, parsed.track, version_to_use),
        source: None,
    })?;

    // Build latest pointer from version record
    let mut latest_item = version_item.clone();
    latest_item.insert("PK".to_string(), AttributeValue::S(latest_pk.to_string()));
    latest_item.insert(
        "SK".to_string(),
        AttributeValue::S(target_latest_sk.clone()),
    );

    // For module/stack promoting to different track, update the track field in the record
    if matches!(kind, CatalogKind::Module | CatalogKind::Stack) && track != parsed.track {
        latest_item.insert("track".to_string(), AttributeValue::S(track.to_string()));
        latest_item.insert(
            "track_version".to_string(),
            AttributeValue::S(version_to_use.to_string()),
        );
    }

    clients
        .dynamodb
        .put_item()
        .table_name(&table)
        .set_item(Some(latest_item))
        .send()
        .await
        .map_err(|e| CatalogError::Storage {
            operation: "promote".to_string(),
            source: anyhow::anyhow!("DynamoDB put failed: {}", e),
        })?;

    Ok(())
}

/// Deprecate: set deprecated=true and persist deprecated_message.
/// Blocks if already deprecated or if this is the latest version.
pub async fn execute_deprecate(
    clients: &AwsClients,
    config: &Config,
    kind: CatalogKind,
    reference: &CatalogRef,
    reason: &str,
) -> Result<(), CatalogError> {
    let parsed = parse_s3_key(kind, &reference.id)?;
    read::zero_pad_semver(&parsed.version, 3).map_err(|e| CatalogError::InvalidInput {
        message: format!("invalid version '{}': {}", parsed.version, e),
        source: Some(anyhow::anyhow!("{}", e)),
    })?;

    let table = match kind {
        CatalogKind::Provider => config.providers_table.clone(),
        CatalogKind::Module => config.modules_table.clone(),
        CatalogKind::Stack => config.stacks_table.clone(),
    };

    // Fetch version record
    let version_selector = catalog_trait::types::VersionSelector::Exact(parsed.version.clone());
    let version_item = ops::execute_get(
        clients,
        config,
        kind,
        &parsed.name,
        &parsed.track,
        &version_selector,
    )
    .await?
    .ok_or_else(|| CatalogError::NotFound {
        kind: format!("{:?}", kind),
        key: format!("{}@{}#{}", parsed.name, parsed.track, parsed.version),
        source: None,
    })?;

    let deprecated = version_item
        .get("deprecated")
        .and_then(|v| v.as_bool().ok())
        .copied()
        .unwrap_or(false);
    if deprecated {
        return Err(CatalogError::InvalidInput {
            message: format!(
                "{} {} version {} is already deprecated",
                format!("{:?}", kind),
                parsed.name,
                parsed.version
            ),
            source: None,
        });
    }

    // Block if this is the latest version
    let latest_selector = catalog_trait::types::VersionSelector::Latest;
    let latest_item = ops::execute_get(
        clients,
        config,
        kind,
        &parsed.name,
        &parsed.track,
        &latest_selector,
    )
    .await?;
    if let Some(latest) = latest_item {
        let latest_version = latest
            .get("version")
            .and_then(|v| v.as_s().ok())
            .map(|s| s.as_str())
            .unwrap_or("");
        if latest_version == parsed.version {
            return Err(CatalogError::InvalidInput {
                message: format!(
                    "Cannot deprecate the latest version ({}) of {} {} in track {}.\n\
                     Please publish a new version that resolves the issue before deprecating this version.",
                    parsed.version,
                    format!("{:?}", kind),
                    parsed.name,
                    parsed.track
                ),
                source: None,
            });
        }
    }

    // Update version record with deprecated=true, deprecated_message
    let mut updated = version_item.clone();
    updated.insert("deprecated".to_string(), AttributeValue::Bool(true));
    updated.insert(
        "deprecated_message".to_string(),
        AttributeValue::S(reason.to_string()),
    );

    clients
        .dynamodb
        .put_item()
        .table_name(&table)
        .set_item(Some(updated))
        .send()
        .await
        .map_err(|e| CatalogError::Storage {
            operation: "deprecate".to_string(),
            source: anyhow::anyhow!("DynamoDB put failed: {}", e),
        })?;

    Ok(())
}

/// Yank: mark entry unavailable in standard listing/get APIs.
pub async fn execute_yank(
    clients: &AwsClients,
    config: &Config,
    kind: CatalogKind,
    reference: &CatalogRef,
) -> Result<(), CatalogError> {
    let parsed = parse_s3_key(kind, &reference.id)?;
    read::zero_pad_semver(&parsed.version, 3).map_err(|e| CatalogError::InvalidInput {
        message: format!("invalid version '{}': {}", parsed.version, e),
        source: Some(anyhow::anyhow!("{}", e)),
    })?;

    let table = match kind {
        CatalogKind::Provider => config.providers_table.clone(),
        CatalogKind::Module => config.modules_table.clone(),
        CatalogKind::Stack => config.stacks_table.clone(),
    };

    // Fetch version record to validate it exists
    let version_selector = catalog_trait::types::VersionSelector::Exact(parsed.version.clone());
    let version_item = ops::execute_get(
        clients,
        config,
        kind,
        &parsed.name,
        &parsed.track,
        &version_selector,
    )
    .await?
    .ok_or_else(|| CatalogError::NotFound {
        kind: format!("{:?}", kind),
        key: format!("{}@{}#{}", parsed.name, parsed.track, parsed.version),
        source: None,
    })?;

    let mut updated = version_item.clone();
    updated.insert("yanked".to_string(), AttributeValue::Bool(true));

    clients
        .dynamodb
        .put_item()
        .table_name(&table)
        .set_item(Some(updated))
        .send()
        .await
        .map_err(|e| CatalogError::Storage {
            operation: "yank".to_string(),
            source: anyhow::anyhow!("DynamoDB put failed: {}", e),
        })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use catalog_trait::types::CatalogKind;

    #[test]
    fn unpad_version_basic() {
        assert_eq!(unpad_version("001.002.003"), "1.2.3");
        assert_eq!(unpad_version("005.045.000"), "5.45.0");
    }

    #[test]
    fn unpad_version_with_prerelease() {
        assert_eq!(unpad_version("001.002.003-alpha.1"), "1.2.3-alpha.1");
    }

    #[test]
    fn parse_provider_s3_key_valid() {
        let r = parse_provider_s3_key("providers/aws/005.045.000/aws.zip").unwrap();
        assert_eq!(r.name, "aws");
        assert_eq!(r.track, "");
        assert_eq!(r.version, "5.45.0");
    }

    #[test]
    fn parse_module_s3_key_valid() {
        let r = parse_module_s3_key("modules/stable/s3bucket/000.001.002/s3bucket.zip").unwrap();
        assert_eq!(r.name, "s3bucket");
        assert_eq!(r.track, "stable");
        assert_eq!(r.version, "0.1.2");
    }

    #[test]
    fn parse_s3_key_provider() {
        let r = parse_s3_key(CatalogKind::Provider, "providers/aws/005.045.000/aws.zip").unwrap();
        assert_eq!(r.name, "aws");
    }

    #[test]
    fn parse_s3_key_module() {
        let r = parse_s3_key(
            CatalogKind::Module,
            "modules/stable/s3bucket/000.001.002/s3bucket.zip",
        )
        .unwrap();
        assert_eq!(r.name, "s3bucket");
        assert_eq!(r.track, "stable");
    }

    #[test]
    fn parse_s3_key_invalid_empty() {
        let e = parse_s3_key(CatalogKind::Provider, "").unwrap_err();
        assert!(matches!(e, CatalogError::InvalidInput { .. }));
    }

    #[test]
    fn parse_s3_key_invalid_provider_format() {
        let e = parse_provider_s3_key("invalid/path").unwrap_err();
        assert!(matches!(e, CatalogError::InvalidInput { .. }));
    }
}
