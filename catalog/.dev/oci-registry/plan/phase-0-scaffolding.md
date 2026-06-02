# Phase 0 — Scaffolding

**Prerequisite**: None (greenfield `catalog/oci-registry` workspace member) · **Architecture**: [architecture-overview.md](../architecture-overview.md), [architecture-traits.md](../architecture-traits.md), [architecture-http.md](../architecture-http.md)

## Goal

Create the `oci-registry` crate skeleton: trait definitions, mock storage, shared HTTP router and error type, per-cloud `[[bin]]` stubs, and `dev/` directory layout — with tests proving the router mounts `/v2/` and handlers depend on traits only (no cloud SDKs in handler code).

## Prerequisites

- [ ] `catalog/oci-registry` added to workspace `Cargo.toml` members
- [ ] Rust toolchain matches repo MSRV / CI
- [ ] Distribution spec v1.1.1 and conformance repo pin documented in `dev/README.md` (reference only until Phase 1)

## Exit criteria

- [ ] `cargo test -p oci-registry` passes (mock trait unit tests + HTTP table tests with zero cloud features)
- [ ] `cargo build --bin oci-registry-local --features local` succeeds (handlers may return **501** for unimplemented routes)
- [ ] `cargo build --bin oci-registry-aws --features aws` and `oci-registry-azure --features azure` succeed without enabling both features in one command
- [ ] No dependency on repo `integration-tests` or `registry-core`
- [ ] `catalog/oci-registry/README.md` lists binaries, features, and “not yet runnable end-to-end” honestly

---

## 0.1 — Workspace crate and feature gates

**Architecture**: [architecture-overview.md](../architecture-overview.md) (crate layout)

**Test layer**: mock (compile-time / `cargo test` package metadata)

### Test first

- [ ] Workspace `cargo metadata` includes package `oci-registry` with features `local`, `aws`, `azure` (mutually exclusive at **binary** level, combinable for `lib` tests only via `mock` / default test cfg)

### Implement

- [ ] `catalog/oci-registry/Cargo.toml`: `[lib]`, `[[bin]]` × 3 with `required-features`
- [ ] `src/lib.rs` exports `Router`, `AppState { bs, rm, config }`, module tree per [plan.md](./plan.md#target-crate-layout-all-phases)
- [ ] Stub `src/bin/local.rs`, `aws.rs`, `azure.rs` each call `lib::router()` + cloud-specific `main` adapter placeholder

### README (when runnable or configurable)

- [ ] `catalog/oci-registry/README.md`: crate purpose, feature flags, “Phase 0 — build only”

---

## 0.2 — Traits and shared types

**Architecture**: [architecture-traits.md](../architecture-traits.md)

**Test layer**: mock

### Test first

- [ ] Unit test: `Digest` parsing `sha256:<hex>` rejects malformed input
- [ ] Unit test: mock `BlobStore::presign_get` returns URL with distinct host from registry base URL (contract for later **307** tests)

### Implement

- [ ] `src/traits.rs`: `BlobStore`, `RegistryMetadata`, `Digest`, `ManifestRecord`, `UploadSession`, `TagPage`, `BlobMeta`, `RegistryError` storage errors
- [ ] `async_trait` on both traits; no AWS/Azure types in trait signatures

### Notes

Object key layout for blobs is implementation detail; document constant in `storage/local.rs` when added in Phase 1.

---

## 0.3 — Mock storage for tests

**Architecture**: [architecture-traits.md](../architecture-traits.md), [guidelines.md — Testability before cloud](../guidelines.md#testability-before-cloud)

**Test layer**: mock

### Test first

- [ ] `MockBlobStore`: `put` then `head` → exists; `presign_get` URL is retrievable via `get_stream` in tests
- [ ] `MockRegistryMetadata`: `put_manifest` + `get_manifest` by tag and by digest; `list_tags` lexical order with `last` cursor

### Implement

- [ ] `src/storage/mock.rs` in-memory `HashMap` backends
- [ ] Test-only helpers to seed repo/tag/manifest without HTTP

---

## 0.4 — RegistryError and OCI JSON errors

**Architecture**: [architecture-http.md](../architecture-http.md#errors)

**Test layer**: mock → HTTP+mock

### Test first

- [ ] `RegistryError::BlobUnknown` serializes to `{"errors":[{"code":"BLOB_UNKNOWN",…}]}` with **404**
- [ ] `RegistryError::Unauthorized` → **401** + `UNAUTHORIZED`

### Implement

- [ ] `RegistryError` enum + `IntoResponse` (or Axum mapper) per spec codes
- [ ] No per-handler ad-hoc JSON shapes

---

## 0.5 — HTTP router shell and path capture

**Architecture**: [architecture-http.md](../architecture-http.md#compute-adapter), [architecture-edge.md](../architecture-edge.md#compute--lambda--functions)

**Test layer**: HTTP + mock traits

### Test first

- [ ] `GET /v2/` with valid test JWT claim → **200** `{}` (capability stub)
- [ ] `GET /v2/` without `Authorization` → **401** (or dev-bypass documented path returns **401** when bypass off)
- [ ] Unregistered route `GET /v2/no/such/route` → **404** OCI error (not plain text)
- [ ] Repository name with slash `GET /v2/org/project/manifests/latest` routes to handler with `name = "org/project"` (path capture test)

### Implement

- [ ] Axum `Router` under `/v2/` with `ANY /v2/{*name}` or equivalent proxy capture
- [ ] `AppState` injects `Arc<dyn BlobStore>`, `Arc<dyn RegistryMetadata>`
- [ ] Handler module stubs return **501** for unimplemented endpoints (distinct from **404** unknown route)

---

## 0.6 — AuthZ claim parser (no edge yet)

**Architecture**: [architecture-auth.md](../architecture-auth.md)

**Test layer**: mock → HTTP+mock

### Test first

- [ ] Parse `infraweave_oci::my/repo::r` → read allowed on `my/repo`
- [ ] `infraweave_oci::my/repo::rw` satisfies write checks
- [ ] Missing claim for repo → **403** on handler authZ gate (unit + HTTP table)

### Implement

- [ ] `src/auth.rs`: claim extraction from JWT payload (test JWT builder in `dev/` later)
- [ ] Separate `authn` (token present / valid signature in local) vs `authz` (repo claim) — edge validates signature in cloud; local validates test secret or bypass flag

### Notes

Production Cognito validation stays at API GW/APIM ([architecture-auth.md](../architecture-auth.md#cognito-jwt-authorizer-production)); local binary may validate HS256 test tokens only.

---

## 0.7 — `dev/` directory scaffolding

**Architecture**: [guidelines-conformance.md](../guidelines-conformance.md), [architecture-overview.md](../architecture-overview.md)

**Test layer**: Setup / Verify (no runtime gate in Phase 0)

### Setup

- [ ] Create `dev/docker-compose.aws.yml`, `dev/docker-compose.azure.yml` (services commented or minimal `hello` — wired in Phases 2–3)
- [ ] Placeholder `dev/bootstrap-aws.sh`, `dev/bootstrap-azure.sh`, `dev/mint-test-jwt.sh` (exit 0 with `echo` usage)
- [ ] Placeholder `dev/run-conformance-*.sh` referencing `OCI_ROOT_URL` ports **5000/5001/5002**
- [ ] `dev/README.md`: ports, profile A/B/C table link to [guidelines-conformance.md](../guidelines-conformance.md#conformance-profiles)

### Verify

- [ ] Scripts are executable and referenced from crate README

---

## 0.8 — CI job skeleton

**Architecture**: [guidelines-conformance.md#ci-workflow-jobs](../guidelines-conformance.md#ci-workflow-jobs)

**Test layer**: Setup / Verify

### Setup

- [ ] GitHub workflow (or extend existing catalog workflow) with job `oci-registry-unit`: `cargo test -p oci-registry` only
- [ ] Conformance jobs commented or `if: false` until Phase 1

### Verify

- [ ] PR runs unit job without AWS/Azure secrets
