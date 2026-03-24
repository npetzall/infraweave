# catalog-trait

`catalog-trait` provides the high-level interface for working with InfraWeave catalog entries:
providers, modules, and stacks.

A catalog entry consists of:
- immutable binary content (for example zip/tar.gz bytes)
- associated metadata (that can evolve over time, independently from the bytes)

## Shared types

Types in `catalog_trait::types` used across multiple traits:

| Type | Description |
|------|-------------|
| `CatalogRef` | Opaque reference to a stored catalog entry (provider/module/stack version). Implementations interpret this as a composite key, versioned identifier, etc. |
| `Metadata` | Single metadata struct for providers, modules and stacks (name, kind, track, version, timestamp, description, reference, cpu, memory, deprecated, deprecated_message). |
| `TerraformInterface` | Unified Terraform-related interface data (tf_variables, tf_outputs, tf_providers, tf_required_providers, tf_lock_providers, tf_extra_environment_variables). |
| `CatalogKind` | Enum: `Provider`, `Module`, `Stack` — what kind of catalog entry to operate on. |
| `VersionSelector` | Enum: `Latest` or `Exact(String)` — how to select a version when fetching. |

---

## `CatalogRead` trait

Read/query capability for providers, modules and stacks. Defined in `src/read.rs`.

### Methods

- **Listing:** `list(kind, query)` — unified entrypoint; `list_providers`, `list_modules`, `list_stacks` — convenience helpers
- **Fetching:** `get(kind, name, track, version)` — unified entrypoint; `get_provider`, `get_module`, `get_stack` — convenience helpers
- **Downloads:** `download_provider`, `download_module`, `download_stack` — binary content for a given `CatalogRef`
- **Attachments:** `list_attachments`, `download_attachment`

### Types (in `catalog_trait::read`)

| Type | Description |
|------|-------------|
| `ProjectionFields` | Bitmask for which heavy fields to populate in list responses (`METADATA`, `MANIFEST`, `TERRAFORM`, `PROVIDER_MIRROR`, `STACK_DATA`, `ALL`). |
| `ContentSource` | Enum: `Url(String)`, `Path(PathBuf)`, `Bytes(Vec<u8>)` — where to read binary content. |
| `Provider` | Full provider entry: `reference`, `metadata`, `manifest`, `terraform` (all optional except `reference`). |
| `Module` | Full module entry: same as provider plus optional `provider_mirror` (map of relative paths to `ContentSource`). |
| `Stack` | Full stack entry: like module plus optional `stack_data` (`ModuleStackData`). |
| `Page<T>` | Pagination envelope: `items`, `next` (continuation token). |
| `Query` | List query: `name`, `track`, `limit`, `next`, `projection`. |
| `CatalogEntry` | Enum: `Provider(Provider)`, `Module(Module)`, `Stack(Stack)`. |

### Shared types used

`CatalogKind`, `CatalogRef`, `Metadata`, `TerraformInterface`, `VersionSelector` — see [Shared types](#shared-types).

---

## `CatalogPopulate` trait

Add new versions and attachments. Defined in `src/populate.rs`.

### Methods

- **Providers:** `add_provider(metadata, manifest, terraform, content)` → `CatalogRef`
- **Modules:** `add_module(metadata, manifest, terraform, content)` → `CatalogRef`
- **Stacks:** `add_stack(metadata, manifest, terraform, stack_data, content)` — `stack_data` is `Option<ModuleStackData>` → `CatalogRef`
- **Attachments:** `add_attachment(reference, name, content)`

### Shared types used

`CatalogRef`, `Metadata`, `TerraformInterface` — see [Shared types](#shared-types).

Manifest and related types are re-exported from `catalog-trait` so implementors need not depend on the `env_defs` crate directly: `ProviderManifest`, `ModuleManifest`, `StackManifest`, `ModuleStackData`, `ModuleResp`, `ProviderResp`, `StackMetadata`, `StackSpec`, `TfLockProvider`, `TfOutput`, `TfRequiredProvider`, `TfVariable`.

---

## `CatalogManagement` trait

Lifecycle operations: promote, deprecate, yank. Defined in `src/management.rs`.

### Methods

- **Unified:** `promote(kind, reference, track, version: Option<&str>)`, `deprecate(kind, reference, reason)`, `yank(kind, reference)`
- **Providers:** `promote_provider`, `deprecate_provider`, `yank_provider`
- **Modules:** `promote_module`, `deprecate_module`, `yank_module`
- **Stacks:** `promote_stack`, `deprecate_stack`, `yank_stack`

### Shared types used

`CatalogKind`, `CatalogRef` — see [Shared types](#shared-types).

---

## `CatalogAvailability` trait

Replication availability and sync across regions. Separate from `Catalog`; implementations can support availability/sync independently of full catalog read/write. Defined in `src/availability.rs`.

### Methods

- **Configured regions:** `configured_regions()` — list regions that can be queried or synced
- **Availability queries:** `availability_provider`, `availability_module`, `availability_stack` — report `<region>: <present|missing>` for a given entity
- **Sync requests:** `sync_provider`, `sync_module`, `sync_stack` — request replication; returns `before`/`after` availability and per-region sync entries

### Types (in `catalog_trait::availability`)

| Type | Description |
|------|-------------|
| `RegionStatus` | Enum: `Present`, `Missing`. |
| `AvailabilityReport` | `regions: Vec<(String, RegionStatus)>` — region → status mapping. |
| `SyncProviderRequest` | `name`, `track`, `version` (`VersionSelector`), `regions`. |
| `SyncModuleRequest` | Same shape as `SyncProviderRequest`. |
| `SyncStackRequest` | Same shape as `SyncProviderRequest`. |
| `SyncEntryStatus` | Enum: `Success`, `Retriable`, `Fatal`. |
| `SyncEntry` | Per-region sync: `source`, `target`, `status`, `error`. |
| `SyncResult` | `before`, `after` (`AvailabilityReport`), `sync` (`Vec<SyncEntry>`). |

### Shared types used

`VersionSelector` — see [Shared types](#shared-types).

---

## `Catalog` trait

Full catalog capability: read + populate + management. Defined in `src/catalog.rs`.

```rust
pub trait Catalog: CatalogRead + CatalogPopulate + CatalogManagement {}
```

A blanket implementation applies for any type that implements all three supertraits. `CatalogAvailability` is optional and separate.

---

## For Implementors

Consumers depend on `catalog-trait` for the interface; runtimes/backends implement the traits (for example [`catalog-aws`](../catalog-aws) for an AWS-backed store).

### HTTP edge (Lambda)

The shared **[`catalog-http`](../catalog-http)** crate exposes the Axum route table for **`/catalog/health`** and **`/catalog/v1/...`** (list, get, download, attachments, management). The **[`catalog-aws-apigw`](../catalog-aws-apigw)** crate wraps that router for **AWS Lambda** with **API Gateway HTTP API** (v2), using **`lambda_http`** to turn proxy integration events into Axum requests. **Read routes** call [`CatalogRead`](src/read.rs) and **management routes** (promote, deprecate, yank per kind) call [`CatalogManagement`](src/management.rs); **populate** (add version / attachment) is not exposed on the edge—use presigned uploads or internal flows. Deployment and payload notes (stage paths, body limits, CORS, authorizer) are in [`catalog-aws-apigw/README.md`](../catalog-aws-apigw/README.md).

### Pagination requirements

Listing APIs return a page envelope `{ items, next }` with `limit` and opaque `next` token. Implementations MUST:

- Order results by a **stable, deterministic sort key**
- Encode `next` as the backend’s “last evaluated key” for that sort
- Return `next` when server-side truncation occurs; omit `next` when the result is complete

### List projection requirements

Listing APIs support typed payload reduction via `Query.projection`:

- `Query.projection == None` → “Full”; implementations SHOULD populate all supported projected fields
- `Query.projection == Some(mask)` → only populate fields in `mask`:
  - `metadata` → `Provider|Module|Stack.metadata` SHOULD be `Some(...)` or `None`
  - `manifest` → `manifest` field
  - `terraform` → `terraform` field
  - `provider_mirror` → `Module|Stack.provider_mirror` (providers do not have this field)
  - `stack_data` → `Stack.stack_data` (Provider/Module do not have this field)

### Replication / availability across deployments

Clients call the catalog in each target region. When an artifact is absent, the backend replicates it rather than requiring the client to download and re-upload. The `CatalogAvailability` trait exposes this: query availability (`<region>: <present|missing>`) and request sync. Actual replication is an implementation detail; callers can treat `get_*` / `download_*` as working consistently from their target environment.
