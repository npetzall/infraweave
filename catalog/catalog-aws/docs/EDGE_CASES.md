# Edge-Case Inventory: Catalog Parity

> Known edge cases to handle for `catalog-aws` parity with `env_aws_direct`.

## 1. Missing Latest Pointer

**Scenario:** No row exists for `PK = LATEST_MODULE` (or `LATEST_STACK`, `LATEST_PROVIDER`) with `SK = MODULE#<track>::<name>`.

**Current behavior:**
- Query returns empty `Items`
- `query_one` in api_common returns `Err(anyhow!("Item not found"))`
- Callers receive "not found" / `None`

**Parity target:** Return `None` or empty list; do not panic. Treat as "no latest version published."

## 2. Malformed Version Key

**Scenario:** Version string fails `zero_pad_semver` (e.g. `"v1.0.0"`, `"1.0"`, `"invalid"`).

**Current behavior:**
- `zero_pad_semver(version, 3).unwrap()` in query builders → **panic** on invalid semver
- No graceful error handling in env_aws_direct API layer

**Parity target:** Return `Err` with clear message (e.g. `InvalidInput` for bad version string). Avoid panic in production path.

## 3. Partial Metadata Records

**Scenario:** DynamoDB item exists but lacks expected fields (e.g. missing `manifest`, `s3_key`, `version`).

**Current behavior:**
- `serde_dynamo::from_item` / `deserialize_module_manifest` may fail or use defaults
- `ModuleResp` has `#[serde(default)]` on many fields; missing fields deserialize to defaults
- `deserialize_module_manifest` expects object for AWS; string for Azure

**Parity target:**
- Preserve `#[serde(default)]` on optional/defaultable fields
- Missing `s3_key` → download fails with "no s3_key"
- Missing `manifest` → deserialize with defaults where possible; document which fields are required

## 4. Empty Track

**Scenario:** `track = ""` for module/stack.

**Current behavior:**
- `get_module_identifier("", "mymod")` → `"::mymod"`
- List queries: `track.is_empty()` → no `begins_with`; query all `PK = LATEST_MODULE`

**Parity target:** Support empty track; treat as "all tracks" for list queries.

## 5. Pagination / ExclusiveStartKey

**Scenario:** `env_aws_direct::read_db_direct` does not support `ExclusiveStartKey`.

**Current behavior:**
- `direct_impl.rs` does not read or pass `ExclusiveStartKey` from query
- Pagination is effectively broken for direct path when using large result sets
- `aws_handlers::read_db` (in-process DynamoDB) supports it and returns `next_token`

**Parity target:** `catalog-aws` must support `ExclusiveStartKey` and return `next_token` (base64-encoded `LastEvaluatedKey`) for full parity.

## 6. Deprecated Latest Version

**Scenario:** The latest version of a module is deprecated.

**Current behavior:**
- `deprecate_module` blocks deprecating the latest version
- Error: "Cannot deprecate the latest version... Please publish a new version that resolves the issue before deprecating this version."

**Parity target:** Enforce same rule: cannot deprecate latest; must publish newer version first.

## 7. Region-Specific Table Names

**Scenario:** `target_region != current_region` for cross-region queries.

**Current behavior:**
- `get_table_name` result is adjusted: `table_name.replace(&current_region, target_region)`
- Same for bucket names in `get_bucket_name_from_env`

**Parity target:** Support region-aware table/bucket resolution when provided.
