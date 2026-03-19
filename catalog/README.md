# Catalog

`catalog` provides the high-level interface for working with InfraWeave catalog entries:
providers, modules, and stacks.

A catalog entry consists of:
- immutable binary content (for example zip/tar.gz bytes)
- associated metadata (that can evolve over time, independently from the bytes)

## Design intent

The crate is centered around the `Catalog` trait, which models common operations across all catalog kinds:

1. Create a new version (metadata + bytes) with `add_*`
2. Evolve entry availability/state via `promote_*`, `deprecate_*`, and `yank_*`
3. Fetch versions (`get_*`) and access the immutable binary content (`download_*`)
4. Perform unified listing queries via `list` / `list_providers` / `list_modules` / `list_stacks`
5. Attach and download additional binary artifacts (for example build info or attestations)

## Key APIs

### `Catalog` trait

Defined in `catalog/src/lib.rs`, it includes methods for:
- Providers: `add_provider`, `promote_provider`, `deprecate_provider`, `yank_provider`, `get_provider`, `download_provider`
- Modules: `add_module`, `promote_module`, `deprecate_module`, `yank_module`, `get_module`, `download_module`
- Stacks: `add_stack`, `promote_stack`, `deprecate_stack`, `yank_stack`, `get_stack`, `download_stack`
- Listing: `list`, plus convenience helpers `list_providers`, `list_modules`, `list_stacks`
- Attachments: `add_attachment`, `list_attachments`, `download_attachment`

### `catalog_types` (shared data structures)

The crate also defines the shared placeholder types used by the `Catalog` trait:
- `CatalogRef` (opaque catalog reference)
- `Metadata` (common catalog metadata)
- `TerraformInterface` (unified view of Terraform-related inputs/outputs)
- `ContentSource` (`Url`, `Path`, or `Bytes`)
- `Provider`, `Module`, `Stack`
- `CatalogKind`, `Query`, `CatalogEntry`
- `VersionSelector` (`Latest` or `Exact(...)`)

## For Implementors

Consumers are expected to depend on `catalog` for the interface, while specific runtimes/backends should implement the `Catalog` trait (for example, the `catalog-aws` crate is intended to host an AWS-backed implementation).

### Pagination Requirements

Catalog listing APIs (`list`, `list_providers`, `list_modules`, `list_stacks`) return a page envelope (`{ items, next }`) supporting pagination via a `limit` and an opaque `next` continuation token.

Implementations MUST ensure pagination is deterministic across calls:
- results MUST be ordered by a **stable, deterministic sort key**.
- the `next` token MUST correspond to (and resume from) the backend’s “last evaluated key” for that sort key.
- when server-side truncation occurs, implementations MUST return `next` so clients can resume; when the result is complete, `next` MUST be absent.

### List Projections Requirements

Listing APIs (`list`, `list_providers`, `list_modules`, `list_stacks`) support typed payload reduction via `catalog_types::Query.projection`.

`Query.projection` semantics (typed contract):
- `Query.projection == None` means "Full" and implementations SHOULD populate all supported projected fields (using `Some(...)` in the corresponding response `Option<...>` fields).
- `Query.projection == Some(mask)` means "Only populate fields included in `mask`":
  - If `mask` includes `metadata`, then `Provider|Module|Stack.metadata` SHOULD be `Some(...)`; otherwise it SHOULD be `None`.
  - If `mask` includes `manifest`, then `Provider|Module|Stack.manifest` SHOULD be `Some(...)`; otherwise it SHOULD be `None`.
  - If `mask` includes `terraform`, then `Provider|Module|Stack.terraform` SHOULD be `Some(...)`; otherwise it SHOULD be `None`.
  - If `mask` includes `stack_data`, then `Stack.stack_data` SHOULD be `Some(...)`; otherwise it SHOULD be `None`.

Notes:
- `Provider` and `Module` do not include `stack_data` in their response types, so the projection naturally cannot request it for those kinds.

### Replication / availability across deployments

The `Catalog` layer is responsible for ensuring catalog data (metadata and associated immutable content) is available across different deployment scopes, such as subscriptions, regions, and availability zones. Any required replication, distribution, or synchronization across those scopes is an implementation concern of the catalog backend, so callers can treat `get_*` / `download_*` as working consistently from their target environment.

