# Behavior Matrix: env_aws_direct Catalog Parity

> Extracted from `env_aws_direct` and `env_aws` API implementations. Target for `catalog-aws` parity.

## 1. List / Get / Download Operations

### 1.1 Provider

| Operation | Query Builder | Key Pattern | Notes |
|-----------|---------------|-------------|-------|
| **List (all latest)** | `get_all_latest_providers_query()` | `PK = LATEST_PROVIDER` | No track filter; no deprecate/dev filters |
| **Get latest** | `get_latest_provider_version_query(provider)` | `PK = LATEST_PROVIDER`, `SK = PROVIDER#<provider>` | Limit 1 |
| **Get exact version** | `get_provider_version_query(provider, version)` | `PK = PROVIDER#<provider>`, `SK = VERSION#<zero-padded>` | Version zero-padded to 3 digits per component |
| **Download** | S3 `get_object` | Key from `s3_key` field of provider record | Bucket from `MODULE_S3_BUCKET` (providers stored in modules bucket) |

### 1.2 Module

| Operation | Query Builder | Key Pattern | Notes |
|-----------|---------------|-------------|-------|
| **List (all latest)** | `get_all_latest_modules_query(track, include_deprecated, include_dev000)` | `PK = LATEST_MODULE`; optional `begins_with(SK, MODULE#<track>::)` | Track filter when non-empty |
| **Get latest** | `get_latest_module_version_query(module, track)` | `PK = LATEST_MODULE`, `SK = MODULE#<track>::<module>` | Module identifier: `{track}::{module}` |
| **Get exact version** | `get_module_version_query(module, track, version)` | `PK = MODULE#<track>::<module>`, `SK = VERSION#<zero-padded>` | `ScanIndexForward: false` for version listing |
| **List versions** | `get_all_module_versions_query(module, track, include_deprecated, include_dev000)` | `PK = MODULE#<track>::<module>`, `begins_with(SK, VERSION#)` | Descending by SK |
| **Download** | S3 `get_object` | Key from `s3_key` field | Bucket from `MODULE_S3_BUCKET` |

### 1.3 Stack

| Operation | Query Builder | Key Pattern | Notes |
|-----------|---------------|-------------|-------|
| **List (all latest)** | `get_all_latest_stacks_query(track, include_deprecated, include_dev000)` | `PK = LATEST_STACK`, same structure as modules | Uses `MODULE#` prefix in SK |
| **Get latest** | `get_latest_stack_version_query(stack, track)` | `PK = LATEST_STACK`, `SK = MODULE#<track>::<stack>` | Same identifier format as module |
| **Get exact version** | `get_stack_version_query(stack, track, version)` | Same as `get_module_version_query` | Stacks stored in same table as modules |
| **List versions** | `get_all_stack_versions_query(stack, track, ...)` | Same as module versions | Same table |

### 1.4 Canonical Key Patterns

- **Latest pointers:** `LATEST_MODULE`, `LATEST_STACK`, `LATEST_PROVIDER`
- **Version rows:** `VERSION#<zero-padded-semver>` (e.g. `VERSION#001.002.003`)
- **Module/Stack identity:** `MODULE#<track>::<name>` via `get_module_identifier(track, name)` → `format!("{}::{}", track, name)`
- **Provider identity:** `PROVIDER#<provider>` (no track)

## 2. Deprecate / Yank Visibility Rules

### 2.1 Filter Expression (when `include_deprecated = false`)

```
attribute_not_exists(deprecated) OR deprecated = :false
```

- `:false` → `false` (boolean)
- Records with `deprecated = true` are excluded by default
- Records with missing `deprecated` attribute are included (defaults to not deprecated)

### 2.2 Dev / Pre-release Filter (when `include_dev000 = false`)

```
NOT begins_with(version, :dev_prefix)
```

- `:dev_prefix` → `"0.0.0-dev"`
- Versions like `0.0.0-dev.1`, `0.0.0-dev+abc` are excluded by default
- Applies to list and list-versions operations

### 2.3 Default Filter Values (API)

- `include_deprecated`: default `false` (exclude deprecated)
- `include_dev000`: default `true` for list modules/stacks/versions (include dev versions)

### 2.4 Yank Semantics

- **Yank** = hard removal; not currently documented in env_aws_direct query layer
- **Deprecate** = soft removal; `deprecated` flag set; existing deployments may continue; new deployments blocked

## 3. Pagination Token Behavior

### 3.1 Input (next_token)

- **Format:** Base64-encoded JSON of DynamoDB `LastEvaluatedKey`
- **Usage:** Decoded and passed as `ExclusiveStartKey` to the next query
- **Source:** `api_common::query_all` decodes `next_token` and sets `query["ExclusiveStartKey"] = key`

### 3.2 Output (next_token)

- **Format:** Base64-encoded JSON of DynamoDB `LastEvaluatedKey`
- **When present:** `result.last_evaluated_key()` is non-empty
- **Implementation:** `aws_handlers::read_db` returns `next_token` when `LastEvaluatedKey` exists
- **env_aws_direct gap:** `read_db_direct` returns raw `LastEvaluatedKey` (JSON object), not base64-encoded `next_token`. Callers must handle both formats for parity.

### 3.3 Limit

- `Limit` passed through from query payload to DynamoDB
- No default limit on list operations; DynamoDB default applies

## 4. Version Zero-Padding

- **Function:** `zero_pad_semver(version, 3)`
- **Example:** `"1.2.3"` → `"001.002.003"`; `"1.2.3-alpha.1"` → `"001.002.003-alpha.1"`
- **Purpose:** Lexicographic sort for DynamoDB range key
- **Error:** Returns `Err` for malformed semver (invalid version string)
