//! DynamoDB query execution and S3 presigning.
//!
//! Builds and executes queries per BEHAVIOR_MATRIX key patterns.

use aws_sdk_dynamodb::types::AttributeValue;
use serde_json::Value as JsonValue;
use std::collections::HashMap;

use crate::client::AwsClients;
use crate::config::Config;
use crate::errors::CatalogError;
use crate::read;

fn json_to_attr(v: &JsonValue) -> Result<AttributeValue, CatalogError> {
    match v {
        JsonValue::String(s) => Ok(AttributeValue::S(s.clone())),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(AttributeValue::N(i.to_string()))
            } else if let Some(f) = n.as_f64() {
                Ok(AttributeValue::N(f.to_string()))
            } else {
                Ok(AttributeValue::S(n.to_string()))
            }
        }
        JsonValue::Bool(b) => Ok(AttributeValue::Bool(*b)),
        JsonValue::Null => Ok(AttributeValue::Null(true)),
        JsonValue::Array(arr) => {
            let list: Result<Vec<_>, _> = arr.iter().map(json_to_attr).collect();
            Ok(AttributeValue::L(list?))
        }
        JsonValue::Object(obj) => {
            let mut map = HashMap::new();
            for (k, v) in obj {
                map.insert(k.clone(), json_to_attr(v)?);
            }
            Ok(AttributeValue::M(map))
        }
    }
}

/// Build list-all-latest query for modules or stacks.
fn list_latest_modules_query(
    pk: &str,
    name: Option<&str>,
    track: Option<&str>,
    include_deprecated: bool,
    include_dev000: bool,
) -> JsonValue {
    let mut query: JsonValue = if let Some(n) = name {
        let sk = format!(
            "MODULE#{}",
            read::get_module_identifier(n, track.unwrap_or(""))
        );
        serde_json::json!({
            "KeyConditionExpression": "PK = :latest AND SK = :sk",
            "ExpressionAttributeValues": { ":latest": pk, ":sk": sk },
        })
    } else if track.map(|t| !t.is_empty()).unwrap_or(false) {
        let t = track.unwrap();
        serde_json::json!({
            "KeyConditionExpression": "PK = :latest AND begins_with(SK, :track)",
            "ExpressionAttributeValues": {
                ":latest": pk,
                ":track": format!("MODULE#{}::", t),
            },
        })
    } else {
        serde_json::json!({
            "KeyConditionExpression": "PK = :latest",
            "ExpressionAttributeValues": { ":latest": pk },
        })
    };

    let mut filters = Vec::new();
    if !include_deprecated {
        filters.push("(attribute_not_exists(deprecated) OR deprecated = :false)");
    }
    filters.push("(attribute_not_exists(yanked) OR yanked = :yanked_false)");
    if !include_dev000 {
        filters.push("NOT begins_with(version, :dev_prefix)");
    }
    if !filters.is_empty() {
        if let Some(obj) = query.as_object_mut() {
            obj.insert(
                "FilterExpression".to_string(),
                JsonValue::String(filters.join(" AND ")),
            );
            if let Some(vals) = obj
                .get_mut("ExpressionAttributeValues")
                .and_then(|v| v.as_object_mut())
            {
                if !include_deprecated {
                    vals.insert(":false".to_string(), JsonValue::Bool(false));
                }
                vals.insert(":yanked_false".to_string(), JsonValue::Bool(false));
                if !include_dev000 {
                    vals.insert(
                        ":dev_prefix".to_string(),
                        JsonValue::String("0.0.0-dev".to_string()),
                    );
                }
            }
        }
    }
    query
}

/// Build list-all-latest query for providers.
fn list_latest_providers_query(name: Option<&str>) -> JsonValue {
    let mut query = if let Some(n) = name {
        let sk = format!("PROVIDER#{}", n);
        serde_json::json!({
            "KeyConditionExpression": "PK = :latest AND SK = :sk",
            "ExpressionAttributeValues": { ":latest": "LATEST_PROVIDER", ":sk": sk, ":yanked_false": false },
        })
    } else {
        serde_json::json!({
            "KeyConditionExpression": "PK = :latest",
            "ExpressionAttributeValues": { ":latest": "LATEST_PROVIDER", ":yanked_false": false },
        })
    };
    if let Some(obj) = query.as_object_mut() {
        obj.insert(
            "FilterExpression".to_string(),
            JsonValue::String(
                "(attribute_not_exists(yanked) OR yanked = :yanked_false)".to_string(),
            ),
        );
    }
    query
}

/// Build get-latest query for module/stack.
fn get_latest_module_query(pk: &str, name: &str, track: &str) -> JsonValue {
    let sk = format!("MODULE#{}", read::get_module_identifier(name, track));
    serde_json::json!({
        "KeyConditionExpression": "PK = :latest AND SK = :sk",
        "ExpressionAttributeValues": { ":latest": pk, ":sk": sk },
        "Limit": 1,
    })
}

/// Build get-latest query for provider.
fn get_latest_provider_query(name: &str) -> JsonValue {
    let sk = format!("PROVIDER#{}", name);
    serde_json::json!({
        "KeyConditionExpression": "PK = :latest AND SK = :sk",
        "ExpressionAttributeValues": { ":latest": "LATEST_PROVIDER", ":sk": sk },
        "Limit": 1,
    })
}

/// Build get-exact-version query for module/stack.
fn get_module_version_query(
    name: &str,
    track: &str,
    version: &str,
) -> Result<JsonValue, CatalogError> {
    let version_padded =
        read::zero_pad_semver(version, 3).map_err(|e| CatalogError::InvalidInput {
            message: format!("invalid version string '{}': {}", version, e),
            source: Some(anyhow::anyhow!("{}", e)),
        })?;
    let pk = format!("MODULE#{}", read::get_module_identifier(name, track));
    let sk = format!("VERSION#{}", version_padded);
    Ok(serde_json::json!({
        "KeyConditionExpression": "PK = :pk AND SK = :sk",
        "ExpressionAttributeValues": { ":pk": pk, ":sk": sk },
        "Limit": 1,
    }))
}

/// Build get-exact-version query for provider.
fn get_provider_version_query(name: &str, version: &str) -> Result<JsonValue, CatalogError> {
    let version_padded =
        read::zero_pad_semver(version, 3).map_err(|e| CatalogError::InvalidInput {
            message: format!("invalid version string '{}': {}", version, e),
            source: Some(anyhow::anyhow!("{}", e)),
        })?;
    let pk = format!("PROVIDER#{}", name);
    let sk = format!("VERSION#{}", version_padded);
    Ok(serde_json::json!({
        "KeyConditionExpression": "PK = :pk AND SK = :sk",
        "ExpressionAttributeValues": { ":pk": pk, ":sk": sk },
        "Limit": 1,
    }))
}

/// Execute a DynamoDB query and return items + last evaluated key.
pub async fn execute_query(
    clients: &AwsClients,
    config: &Config,
    kind: catalog_trait::types::CatalogKind,
    query: &JsonValue,
    exclusive_start_key: Option<HashMap<String, AttributeValue>>,
) -> Result<
    (
        Vec<HashMap<String, AttributeValue>>,
        Option<HashMap<String, AttributeValue>>,
    ),
    CatalogError,
> {
    let table = config.table_for_kind(kind);
    let mut builder = clients.dynamodb.query().table_name(table);

    if let Some(key_condition) = query.get("KeyConditionExpression") {
        if let Some(expr) = key_condition.as_str() {
            builder = builder.key_condition_expression(expr);
        }
    }
    if let Some(filter_expr) = query.get("FilterExpression") {
        if let Some(expr) = filter_expr.as_str() {
            builder = builder.filter_expression(expr);
        }
    }
    if let Some(attr_values) = query.get("ExpressionAttributeValues") {
        if let Some(obj) = attr_values.as_object() {
            for (key, value) in obj {
                builder = builder.expression_attribute_values(key, json_to_attr(value)?);
            }
        }
    }
    if let Some(limit) = query.get("Limit") {
        if let Some(num) = limit.as_i64() {
            builder = builder.limit(num as i32);
        }
    }
    if let Some(scan_forward) = query.get("ScanIndexForward") {
        if let Some(val) = scan_forward.as_bool() {
            builder = builder.scan_index_forward(val);
        }
    }
    if let Some(key) = exclusive_start_key {
        if !key.is_empty() {
            builder = builder.set_exclusive_start_key(Some(key));
        }
    }

    let result = builder.send().await.map_err(|e| CatalogError::Storage {
        operation: "query".to_string(),
        source: anyhow::anyhow!("DynamoDB query failed: {}", e),
    })?;

    let items: Vec<HashMap<String, AttributeValue>> = result.items().iter().cloned().collect();

    let last = result.last_evaluated_key().cloned();

    Ok((items, last))
}

/// Execute list query for the given kind.
pub async fn execute_list(
    clients: &AwsClients,
    config: &Config,
    kind: catalog_trait::types::CatalogKind,
    query: &catalog_trait::read::Query,
) -> Result<
    (
        Vec<HashMap<String, AttributeValue>>,
        Option<HashMap<String, AttributeValue>>,
    ),
    CatalogError,
> {
    let next_key = query
        .next
        .as_ref()
        .map(|t| read::decode_next_token(t))
        .transpose()?;

    let limit = query.limit.map(|l| l as i64);

    let query_json = match kind {
        catalog_trait::types::CatalogKind::Provider => {
            let mut q = list_latest_providers_query(query.name.as_deref());
            if let Some(l) = limit {
                q["Limit"] = serde_json::json!(l);
            }
            q
        }
        catalog_trait::types::CatalogKind::Module => {
            let track = query.track.as_deref();
            let mut q = list_latest_modules_query(
                "LATEST_MODULE",
                query.name.as_deref(),
                track,
                false, // include_deprecated default
                true,  // include_dev000 default
            );
            if let Some(l) = limit {
                q["Limit"] = serde_json::json!(l);
            }
            q
        }
        catalog_trait::types::CatalogKind::Stack => {
            let track = query.track.as_deref();
            let mut q = list_latest_modules_query(
                "LATEST_STACK",
                query.name.as_deref(),
                track,
                false,
                true,
            );
            if let Some(l) = limit {
                q["Limit"] = serde_json::json!(l);
            }
            q
        }
    };

    execute_query(clients, config, kind, &query_json, next_key).await
}

/// Execute get for latest or exact version.
pub async fn execute_get(
    clients: &AwsClients,
    config: &Config,
    kind: catalog_trait::types::CatalogKind,
    name: &str,
    track: &str,
    version: &catalog_trait::types::VersionSelector,
) -> Result<Option<HashMap<String, AttributeValue>>, CatalogError> {
    let query_json = match version {
        catalog_trait::types::VersionSelector::Latest => match kind {
            catalog_trait::types::CatalogKind::Provider => get_latest_provider_query(name),
            catalog_trait::types::CatalogKind::Module => {
                get_latest_module_query("LATEST_MODULE", name, track)
            }
            catalog_trait::types::CatalogKind::Stack => {
                get_latest_module_query("LATEST_STACK", name, track)
            }
        },
        catalog_trait::types::VersionSelector::Exact(v) => match kind {
            catalog_trait::types::CatalogKind::Provider => get_provider_version_query(name, v)?,
            catalog_trait::types::CatalogKind::Module
            | catalog_trait::types::CatalogKind::Stack => get_module_version_query(name, track, v)?,
        },
    };

    let (items, _) = execute_query(clients, config, kind, &query_json, None).await?;
    Ok(items.into_iter().next())
}
