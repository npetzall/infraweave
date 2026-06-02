# Phase 0 — Traits, types, local backends

**Parent**: [`plan.md`](./plan.md) · **Architecture**: [`architecture-traits.md`](../architecture-traits.md), [`architecture-overview.md`](../architecture-overview.md)

## Goal

Establish the `oci-registry` library foundation: domain types, OCI error JSON, `BlobStore` / `RegistryMetadata` traits, digest verification, and **local** implementations sufficient for unit and storage tests — **no** HTTP surface yet.

## Prerequisites

- [ ] Create `catalog/oci-registry/` and add `"catalog/oci-registry"` to root workspace `members`
- [ ] `catalog/oci-registry/Cargo.toml`: `lib` + `[[bin]]` stubs (one cloud feature per binary); features `aws`, `azure`, `local` (mutually exclusive per bin via `required-features`)
- [ ] Create **`catalog/oci-registry/README.md`** (crate root) — stub with workspace path, `cargo test -p oci-registry`, link to `catalog/.dev/oci-registry/plan-prearch/plan.md`
- [ ] `catalog/oci-registry/dev/README.md` stub — pointer to crate README for commands; phase plans for compose/SAM detail (Phase 1/2)
- [ ] Confirm **no** dependency on `integration-tests` crate
- [ ] CI runs `cargo test -p oci-registry`

## Exit criteria

- [ ] `cargo test -p oci-registry` passes (unit + trait storage tests)
- [ ] In-memory mocks implement both traits for downstream HTTP tests (Phase 1)
- [ ] Every `BlobStore::put` and upload-complete path rejects digest mismatch
- [ ] Blob keys match `v2/blobs/sha256/<aa>/<hex>/data`

---

## 0.1 — Workspace and conformance harness stub

### Test first

- [ ] `oci-registry` `lib` + placeholder `oci-registry-local` bin; `cargo test -p oci-registry --features local` succeeds
- [ ] Add dev-dependency or script doc linking [distribution-spec conformance](https://github.com/opencontainers/distribution-spec/tree/v1.1.1/conformance) — **not** required green in Phase 0; profiles A/B wired in Phase 1 per [`guidelines-conformance.md`](../guidelines-conformance.md)

### Implement

- [ ] `Cargo.toml`: `async-trait`, `thiserror`, `serde`, `serde_json`, `sha2`, `uuid`, `chrono` (or minimal set)
- [ ] README: **Building** (`cargo build -p oci-registry`), **Features** (`local` | `aws` | `azure` | `gcp`), **Testing** (`cargo test`, feature flags)
- [ ] README: link to design docs in `catalog/.dev/oci-registry/` and distribution spec v1.1.1 as conformance source of truth

---

## 0.2 — Domain types and digest

### Test first

- [ ] `Digest::parse("sha256:…")` accepts valid; rejects malformed algorithm, odd length, missing prefix
- [ ] `Digest::verify(bytes)` matches computed sha256 hex
- [ ] `RepositoryName` validation: rejects empty, invalid path segments per distribution name rules
- [ ] `RegistryError` serializes to OCI `{"errors":[{"code","message",…}]}` shape

### Implement

- [ ] `Digest`, `RepositoryName`, `BlobMeta`, `ManifestRecord`, `UploadSession`, `TagPage` types ([`architecture-traits.md`](../architecture-traits.md))
- [ ] `RegistryError` + `IntoResponse` stub (for Phase 1 HTTP)

---

## 0.3 — Trait definitions

### Test first

- [ ] Compile-only test or trait object test: mock type implements `BlobStore` + `RegistryMetadata` with `unimplemented!` — ensures trait bounds (`Send + Sync`) compile

### Implement

- [ ] `BlobStore`: `head`, `get_stream`, `put`, `delete`, `presign_get`, `presign_put` ([`architecture-traits.md`](../architecture-traits.md))
- [ ] `RegistryMetadata`: manifest CRUD, tags, upload session, `link_blob_to_repo`, `list_referrers`
- [ ] Document: trait methods stay cloud-agnostic (GCP backend deferred to Phase 4)

---

## 0.4 — In-memory backends (mocks)

### Test first

- [ ] `InMemoryBlobStore`: `put` then `head` returns size; second `put` same digest is idempotent
- [ ] `InMemoryBlobStore`: `put` with wrong digest returns error
- [ ] `InMemoryRegistryMetadata`: `put_manifest` + `get_manifest` by tag and by digest string
- [ ] `list_tags` returns lexical order; pagination with `last` cursor is stable across calls
- [ ] Upload session: `create_upload` → `update_upload_range` → `complete_upload` updates offset and completes

### Implement

- [ ] `InMemoryBlobStore` behind `Arc<RwLock<…>>` or equivalent
- [ ] `InMemoryRegistryMetadata` with repo partition map
- [ ] Export as `oci_registry::testing` or `#[cfg(test)]` module for Phase 1 router tests

---

## 0.5 — Local filesystem `BlobStore`

### Test first

- [ ] Temp dir: `put` writes file at expected relative key (`v2/blobs/sha256/…/data`)
- [ ] `head` missing digest → not found
- [ ] `delete` removes object; `head` afterward → not found
- [ ] `presign_get` returns `file://` or http URL suitable for local tests (MinIO in Phase 1 if needed)

### Implement

- [ ] `LocalFsBlobStore { root: PathBuf }`
- [ ] Key builder function shared with AWS impl later

---

## 0.6 — Local SQLite `RegistryMetadata`

### Test first

- [ ] Round-trip: `put_manifest` for tag `latest` + digest row; `get_manifest` by tag returns `TargetDigest` and payload bytes
- [ ] `link_blob_to_repo` + query used later for GC graph (store `References` on digest row)
- [ ] Concurrent `put_manifest` for two tags same digest — both readable (read-optimized duplication strategy preview)

### Implement

- [ ] Schema: `PK`/`SK` equivalent or normalized tables mirroring [`architecture-backends.md`](../architecture-backends.md) DynamoDB shape
- [ ] `list_tags` uses `ORDER BY tag ASC` with `last` filter

---

## 0.7 — Digest enforcement on all writes

### Test first

- [ ] `BlobStore::put` with stream whose hash ≠ claimed digest → error, no file left on disk
- [ ] `complete_upload` path helper (orchestration stub) rejects if session bytes hash ≠ `?digest=`

### Implement

- [ ] Shared `verify_digest(bytes, digest)` used by all backends
- [ ] Document non-obvious HTTP behavior in [`architecture-flows.md`](../architecture-flows.md) (or phase plan) if new cases found

---

## README — Phase 0

- [ ] `catalog/oci-registry/README.md` exists and documents: workspace member, crate layout, `cargo build` / `cargo test`, feature flags, bin names (stubs OK)
- [ ] Any new `[[bin]]` or feature added in this phase is listed in README the same PR

---

## Phase 0 — Done checklist

- [ ] All sections above checked
- [ ] README checklist (above) complete
- [ ] No HTTP routes required
- [ ] Ready for [`plan-1.md`](./plan-1.md) (Axum router + AWS backends)
