# Phase 1 — Local backends (profile A)

**Prerequisite**: [phase-0-scaffolding.md](./phase-0-scaffolding.md) exit criteria · **Architecture**: [architecture-backends.md](../architecture-backends.md) (local), [architecture-flows.md](../architecture-flows.md), [architecture-auth.md](../architecture-auth.md)

## Goal

Implement `FsBlobStore` + `SqliteRegistryMetadata`, wire `oci-registry-local`, and deliver spec-compliant **read and push** flows on filesystem + SQLite so **conformance profile A** (`http://127.0.0.1:5000`) passes with `OCI_TEST_CONTENT_MANAGEMENT=0`. Blob pull uses **307** to local presigned/file URLs (same handler code path as cloud).

## Prerequisites

- [ ] Phase 0 complete
- [ ] `dev/mint-test-jwt.sh` mints `infraweave_oci::conformance/test::rw` (or documented `OCI_AUTH_BYPASS` for dev only)
- [ ] Go toolchain available for conformance job (Phase 1 exit)

## Exit criteria

- [ ] `cargo run --bin oci-registry-local --features local` serves `:5000`
- [ ] `./dev/run-conformance-local.sh` green with `OCI_TEST_PULL=1`, `OCI_TEST_PUSH=1`, `OCI_TEST_CONTENT_DISCOVERY=1`, `OCI_TEST_CONTENT_MANAGEMENT=0`
- [ ] CI job `conformance-local`: unit tests → profile A
- [ ] `oci_tag_download` structured log on successful manifest GET by tag ([architecture-observability.md](../architecture-observability.md))
- [ ] DELETE endpoints may return **405** (acceptable until Phase 4)

---

## 1.1 — Local `BlobStore` (filesystem)

**Architecture**: [architecture-traits.md](../architecture-traits.md), [architecture-backends.md](../architecture-backends.md)

**Test layer**: mock → local trait tests

### Test first

- [ ] `put` writes `v2/blobs/sha256/<2>/<full>/data` under configurable root
- [ ] `head` missing digest → not found
- [ ] `presign_get` returns URL that serves same bytes as `get_stream` (file:// or short-lived local HTTP — document choice)

### Implement

- [ ] `src/storage/local.rs`: `FsBlobStore`
- [ ] Wire in `oci-registry-local` binary only

### README (when runnable or configurable)

- [ ] Document `OCI_REGISTRY_DATA_DIR`, blob layout, presign TTL for local

---

## 1.2 — Local `RegistryMetadata` (SQLite)

**Architecture**: [architecture-backends.md](../architecture-backends.md), [architecture-traits.md](../architecture-traits.md)

**Test layer**: local trait tests

### Test first

- [ ] `put_manifest` creates `TAG#` + `DIGEST#` rows; `get_manifest` by tag returns `TargetDigest` for header
- [ ] `list_tags` returns lexical order; pagination with `last` stable across inserts
- [ ] `create_upload` / `complete_upload` session lifecycle
- [ ] `list_referrers` empty → empty image index (not **404**)

### Implement

- [ ] SQLite schema mirroring PK/SK (`REPO#…`, `TAG#`, `DIGEST#`, `UPLOAD#`) and `SubjectDigest` for referrers
- [ ] Atomic manifest write in transaction (simulate DynamoDB transact semantics)

### Notes

Manifest gzip + item budget: use DynamoDB **4096** B budget in local tests to catch oversize early ([architecture-backends.md#manifest-payload-size-limit](../architecture-backends.md#manifest-payload-size-limit)).

---

## 1.3 — Capability check `GET /v2/`

**Spec**: [Base](https://github.com/opencontainers/distribution-spec/blob/v1.1.1/spec.md#base)
**Architecture**: [architecture-flows.md#capability-check](../architecture-flows.md#capability-check)
**Test layer**: HTTP + mock → local

### Test first

- [ ] Valid JWT → **200** `{}`
- [ ] Missing/invalid token → **401**

### Implement

- [ ] Handler uses auth only (no storage)

---

## 1.4 — Pull manifest by tag and digest

**Spec**: [GET Manifest](https://github.com/opencontainers/distribution-spec/blob/v1.1.1/spec.md#get-manifest)
**Architecture**: [architecture-flows.md#pull-manifest-by-tag-tag-download](../architecture-flows.md#pull-manifest-by-tag-tag-download)
**Test layer**: HTTP + mock → local

### Test first

- [ ] `GET /v2/{name}/manifests/{tag}` with `::r` → **200**, `Docker-Content-Digest` equals `TargetDigest` (raw bytes hash, not gzip)
- [ ] Unknown tag → **404** `MANIFEST_UNKNOWN`
- [ ] `GET …/manifests/sha256:…` by digest path works
- [ ] Successful tag GET emits `oci_tag_download` log field (repo, tag) — no manifest body in log

### Implement

- [ ] Handler: authZ → `rm.get_manifest` → response headers + body
- [ ] Local RM returns decompressed or stored payload per design (header still from `TargetDigest`)

---

## 1.5 — Pull blob via 307

**Spec**: [GET Blob](https://github.com/opencontainers/distribution-spec/blob/v1.1.1/spec.md#get-blob)
**Architecture**: [architecture-flows.md#pull-blob-layer](../architecture-flows.md#pull-blob-layer)
**Test layer**: HTTP + mock → local

### Test first

- [ ] `GET /v2/{name}/blobs/{digest}` with `::r` → **307**, `Location` host ≠ registry host, `Docker-Content-Digest` matches digest
- [ ] `HEAD` same path → documented behavior (**307** or **200** — match spec + tests)
- [ ] Missing blob → **404** `BLOB_UNKNOWN`
- [ ] Mock **BS** `presign_get` called once; handler does not stream blob bytes in response body

### Implement

- [ ] Handler: authZ → optional `bs.head` → `bs.presign_get` → **307**
- [ ] `FsBlobStore::presign_get` for local profile A

### README (when runnable or configurable)

- [ ] Document: clients must not send `Authorization` to presigned `Location` URLs

---

## 1.6 — List tags

**Spec**: [List Tags](https://github.com/opencontainers/distribution-spec/blob/v1.1.1/spec.md#list-tags)
**Architecture**: [architecture-flows.md#list-tags](../architecture-flows.md#list-tags)
**Test layer**: HTTP + mock → local

### Test first

- [ ] `GET …/tags/list` → **200** JSON, lexical order
- [ ] `n` and `last` query params paginate correctly

### Implement

- [ ] Handler → `rm.list_tags`

---

## 1.7 — Referrers API

**Spec**: [Listing Referrers](https://github.com/opencontainers/distribution-spec/blob/v1.1.1/spec.md#listing-referrers)
**Architecture**: [architecture-flows.md#referrers-oci-v11](../architecture-flows.md#referrers-oci-v11)
**Test layer**: HTTP + mock → local

### Test first

- [ ] `GET …/referrers/{digest}` with no referrers → **200** empty index (not **404**)
- [ ] After pushing attestation manifest with `subject`, listing includes descriptor
- [ ] Optional `?artifactType=` filter

### Implement

- [ ] Handler → `rm.list_referrers`; SQLite query on `SubjectDigest`

---

## 1.8 — Registry-hosted blob upload

**Spec**: [Pushing a blob](https://github.com/opencontainers/distribution-spec/blob/v1.1.1/spec.md#pushing-a-blob)
**Architecture**: [architecture-flows.md#registry-hosted-upload](../architecture-flows.md#registry-hosted-upload)
**Test layer**: HTTP + mock → local

### Test first

- [ ] `POST …/blobs/uploads/` → **202**, `Location` uses registry host (`REGISTRY_PUBLIC_URL`)
- [ ] `PATCH` + `PUT ?digest=` → **201**, blob in `BlobStore`, `link_blob_to_repo`
- [ ] Wrong `Content-Range` → **416**
- [ ] `PUT ?digest=` verifies digest (mismatch → error)

### Implement

- [ ] Upload handlers: **RM** session + **BS** `put` through local server (no presigned offload required for profile A unless conformance size forces it)

### README (when runnable or configurable)

- [ ] Document upload size limits for local server vs future API GW cap

---

## 1.9 — Push manifest

**Spec**: [PUT Manifest](https://github.com/opencontainers/distribution-spec/blob/v1.1.1/spec.md#put-manifest)
**Architecture**: [architecture-flows.md#push-manifest--tag](../architecture-flows.md#push-manifest--tag)
**Test layer**: HTTP + mock → local

### Test first

- [ ] `PUT …/manifests/{ref}` with `::rw` → **201** + `Docker-Content-Digest`
- [ ] Missing layer in **BS** → **400** / `BLOB_UNKNOWN` per spec mapping
- [ ] Gzip manifest over item budget → **413** / `SIZE_INVALID`

### Implement

- [ ] Validate JSON → gzip once → `bs.head` each ref → `rm.put_manifest` transact

---

## 1.10 — Blob mount

**Spec**: [Mount](https://github.com/opencontainers/distribution-spec/blob/v1.1.1/spec.md#mounting-a-blob-from-another-repository)
**Architecture**: [architecture-flows.md#blob-mount](../architecture-flows.md#blob-mount)
**Test layer**: HTTP + mock → local

### Test first

- [ ] Mount existing digest → **201** without re-upload
- [ ] Unknown digest → **404** (no **202** fallback)
- [ ] Missing `::r` on `from` or `::rw` on dest → **403**

### Implement

- [ ] `bs.head` + `rm.link_blob_to_repo`

---

## 1.11 — Conformance profile A gate

**Architecture**: [guidelines-conformance.md](../guidelines-conformance.md)

**Test layer**: conformance (profile A)

### Test first

- [ ] `./dev/run-conformance-local.sh` exits 0 against `:5000`

### Implement

- [ ] Pin conformance `@v1.1.1`; env `OCI_NAMESPACE=conformance/test`, auth via minted JWT
- [ ] Fix failures iteratively (TDD per failing test name)

### README (when runnable or configurable)

- [ ] Crate README: full command to start local + run conformance

---

## 1.12 — CI `conformance-local` job

**Test layer**: Setup / Verify

### Setup

- [ ] Enable workflow job: `cargo test` → start `oci-registry-local` → `run-conformance-local.sh`

### Verify

- [ ] PR passes without cloud credentials
