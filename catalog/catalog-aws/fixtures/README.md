# Golden Fixtures for Catalog Parity

Representative records used for acceptance tests and snapshot parity validation.

## Fixture Descriptions

| Fixture | Kind | Purpose |
|---------|------|---------|
| `provider_record.json` | Provider | Canonical provider (aws 5.45.0) |
| `module_record.json` | Module | Canonical module (s3bucket 0.1.2, stable) |
| `stack_record.json` | Stack | Canonical stack (bucketcollection 0.2.0) with stack_data.modules |
| `module_deprecated.json` | Module | Deprecated module version (deprecated=true, deprecated_message) |
| `module_dev_version.json` | Module | Pre-release version (0.0.0-dev.1) for dev filter testing |

## DynamoDB Key Mapping

For version records:
- **Provider:** `PK = PROVIDER#<name>`, `SK = VERSION#<zero-padded>`
- **Module:** `PK = MODULE#<track>::<module>`, `SK = VERSION#<zero-padded>`
- **Stack:** Same as module; `PK = MODULE#<track>::<stack>`, `SK = VERSION#<zero-padded>`

For latest pointer records:
- **Provider:** `PK = LATEST_PROVIDER`, `SK = PROVIDER#<name>`
- **Module:** `PK = LATEST_MODULE`, `SK = MODULE#<track>::<module>`
- **Stack:** `PK = LATEST_STACK`, `SK = MODULE#<track>::<stack>`

## Usage

These fixtures are loaded in acceptance tests to validate:
1. Deserialization into `catalog_aws::compat_models` (`ProviderResp`, `ModuleResp`)
2. `tests/compat_roundtrip.rs`: fixture → read mapping → `compat` legacy adapters yields the same compat structs (`PartialEq`)
3. Filter behavior (deprecated, dev) produces expected results
