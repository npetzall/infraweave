# Phase 2 — Upload offload, mount, referrers, Azure

**Parent**: [`plan.md`](./plan.md) · **Prerequisite**: [`plan-1.md`](./plan-1.md) complete · **Architecture**: [`architecture-flows.md`](../architecture-flows.md), [`architecture-backends.md`](../architecture-backends.md), [`architecture-cost.md`](../architecture-cost.md)

## Goal

Reduce Lambda pressure on large uploads (Phase 2 presigned upload), add **blob mount** and **referrers** API, and ship **`oci-registry-azure`** at feature parity with AWS for Phase 1 endpoints plus Phase 2 additions.

## Exit criteria

- [ ] Large blob push uses presigned PUT on **202** `Location`; terminal `PUT ?digest=` on **registry host** validates digest and returns OCI **201**
- [ ] `POST ?mount=&from=` works with dual-repo JWT claims
- [ ] `GET …/referrers/{digest}` returns OCI image index JSON
- [ ] **Conformance profile C**: distribution-spec suite green vs `oci-registry-azure` on local Azurite + Cosmos emulator (§2.0, [`guidelines-conformance.md`](../guidelines-conformance.md))
- [ ] `oci-registry-azure` passes same HTTP/integration tests as AWS against §2.0 stack
- [ ] Phase 1 conformance profiles A + B still green

---

## 2.0 — Local Azure dev environment (Azurite, Cosmos emulator, Functions)

Introduced with Azure implementation in this phase. Same principles as Phase 1 §1.0: everything under **`catalog/oci-registry/dev/`**, no `integration-tests` dependency.

| Component | Image / tool | Role |
|-----------|----------------|------|
| **Azurite** | `mcr.microsoft.com/azure-storage/azurite` | `BlobStore` — blob + SAS (**307**) |
| **Cosmos DB emulator** | `mcr.microsoft.com/cosmosdb/linux/azure-cosmos-emulator` | `RegistryMetadata` — container `manifests` per [`architecture-backends.md`](../architecture-backends.md) |
| **Azure Functions Core Tools** | `func` on host (optional) | Local HTTP for `oci-registry-azure` custom handler |
| **Direct Axum (recommended for conformance)** | `cargo run --bin oci-registry-azure` | Simpler than full APIM locally; same `lib` handlers |

**Conformance profile C** (`azure` feature): [`guidelines-conformance.md`](../guidelines-conformance.md) — port **5002** (avoid collision with :5000 local, :5001 AWS).

### Setup (document + automate)

- [ ] `catalog/oci-registry/dev/docker-compose.azure.yml`: `azurite` (blob port **10000**), `cosmos` (emulator **8081**); shared network
- [ ] `catalog/oci-registry/dev/bootstrap-azure.sh`: create blob container; Cosmos database + container `manifests`; partition key `/repository`
- [ ] Document env (host → emulators):

  | Variable | Example | Purpose |
  |----------|---------|---------|
  | `AZURE_STORAGE_CONNECTION_STRING` | Azurite connection string (see `dev/README.md`) | Blob SDK |
  | `COSMOS_ENDPOINT` | `https://127.0.0.1:8081` | Cosmos SDK (emulator TLS; document `--disable-tls` or cert trust) |
  | `COSMOS_KEY` | Emulator well-known key | Cosmos auth |
  | `OCI_BLOB_CONTAINER` | `oci-registry` | Blob container name |
  | `OCI_COSMOS_DATABASE` | `oci-registry` | Database id |
  | `OCI_AUTH_BYPASS` | `1` (dev only) | Local authZ without APIM |

- [ ] Optional: `catalog/oci-registry/dev/host.json` + `local.settings.json` for `func host start` (custom handler → `oci-registry-azure` binary)
- [ ] Document [Azure Functions Core Tools](https://learn.microsoft.com/en-us/azure/azure-functions/functions-run-local) install

### Test first (against local Azure stack)

- [ ] Compose up: `AzureBlobStore` put/head at `v2/blobs/sha256/…/data`; SAS GET without `Authorization`
- [ ] `CosmosRegistryMetadata` round-trip tag + digest documents; transactional batch on manifest put
- [ ] `cargo run --bin oci-registry-azure --features azure` (or `func host start`): `GET /v2/` → **200**; blob **307** to Azurite SAS
- [ ] Conformance profile C: `dev/run-conformance-azure.sh` green

### Implement

- [ ] `docker-compose.azure.yml` + `bootstrap-azure.sh` + `env.azure.json` for Functions/SAM-equivalent
- [ ] `catalog/oci-registry/dev/run-conformance-azure.sh`: compose → bootstrap → start `oci-registry-azure` on **:5002** → Go conformance → teardown
- [ ] `catalog/oci-registry/tests/azure_local.rs` (behind `azure` feature): script-driven or oci-registry-only testcontainers — **not** `integration-tests`
- [ ] **`catalog/oci-registry/README.md`**: **Local Azure**, conformance profile C, port :5002, `dev/run-conformance-azure.sh`, `docker-compose.azure.yml`, `bootstrap-azure.sh`
- [ ] `dev/README.md`: emulator limits (Cosmos feature gaps, cert trust on macOS/Linux); crate README links here
- [ ] CI job `conformance-azure` per [`guidelines-conformance.md`](../guidelines-conformance.md)

### Notes

- **APIM** JWT policies are not emulated locally; use `OCI_AUTH_BYPASS` or test JWT middleware in the Azure bin for conformance (production uses APIM JWT validation).
- Emulator transactional batch / referrers queries: validate early; document workarounds in [`architecture-flows.md#referrers-oci-v11`](../architecture-flows.md#referrers-oci-v11) or this plan if needed.
- §2.6–2.8 implement against this stack; do not wire Azure storage tests through `integration-tests`.

---

## 2.1 — Presigned upload threshold

### Test first

- [ ] Config `PRESIGN_UPLOAD_MIN_BYTES`: blob size above threshold → **202** `Location` is presigned PUT host (S3), not registry
- [ ] Below threshold → Phase 1 behavior (registry session URL)
- [ ] Client completes bytes on S3, then `PUT …/uploads/{uuid}?digest=` on registry → **201** + `Docker-Content-Digest`
- [ ] Direct `PUT ?digest=` to S3 only (skip registry commit) → blob **not** visible in registry pull (proves commit required)

### Implement

- [ ] `BlobStore::presign_put` wired in upload `POST` handler
- [ ] Commit handler: `HeadObject` / size check + digest verify + `complete_upload` ([`architecture-flows.md#phase-2-upload-offload`](../architecture-flows.md#phase-2-upload-offload))

---

## 2.2 — S3 multipart / chunked PATCH without full buffer

### Test first

- [ ] PATCH sequence maps to S3 multipart parts; final `PUT ?digest=` completes multipart
- [ ] PATCH wrong `Content-Range` → **416**
- [ ] Part size below `OCI-Chunk-Min-Length` (when declared) → **416** except final chunk

### Implement

- [ ] Multipart helpers on `S3BlobStore` (init / upload_part / complete)
- [ ] Azure: block list equivalent in §2.7

---

## 2.3 — Redirect safety tests

### Test first

- [ ] Integration: client does not forward `Authorization` to presigned host (**403** if it does — document)
- [ ] Presigned PUT without `Content-Length` when length was declared on `POST` → rejected at presign time

### Implement

- [ ] Operator doc snippet: do not forward `Authorization` to presigned blob URLs ([`architecture-flows.md`](../architecture-flows.md#pull-blob-layer))

---

## 2.4 — Blob mount (end-11)

### Test first

- [ ] `POST …/uploads/?mount=sha256:…&from=other/repo` with JWT `other/repo::r` + target `::rw` → **201**
- [ ] Source blob missing → **404**
- [ ] Missing `from` read claim → **403**
- [ ] Digest already in pool → **201** without S3 copy (metadata link only)

### Implement

- [ ] Handler: `BlobStore::head` + `RegistryMetadata::link_blob_to_repo`
- [ ] No duplicate S3 object

---

## 2.5 — Referrers index + API (end-12)

### Test first

- [ ] After pushing attestation manifest with `subject` digest, `GET …/referrers/{digest}` returns image index with descriptor
- [ ] `?artifactType=` filter reduces entries
- [ ] Unknown subject → empty index or **404** per spec choice (test locks behavior)

### Implement

- [ ] AWS: `GSI_Referrers` query (`SubjectDigest`, `PK`) — [`architecture-backends.md`](../architecture-backends.md), [`architecture-cost.md`](../architecture-cost.md#referrers-gsi-aws)
- [ ] `put_manifest` sets `SubjectDigest` on referrer rows
- [ ] `RegistryMetadata::list_referrers`

---

## 2.6 — Azure `BlobStore`

Uses **Azurite** from [§2.0](#20--local-azure-dev-environment-azurite-cosmos-emulator-functions).

### Test first

- [ ] Against §2.0 Azurite: same key layout `v2/blobs/sha256/…/data`
- [ ] `presign_get` / `presign_put` SAS URLs work without Authorization

### Implement

- [ ] `AzureBlobStore` feature module
- [ ] Wire in `oci-registry-azure` bin (`azure` feature, same crate as AWS)

---

## 2.7 — Azure `RegistryMetadata` (Cosmos)

Uses **Cosmos emulator** from [§2.0](#20--local-azure-dev-environment-azurite-cosmos-emulator-functions).

### Test first

- [ ] Against §2.0 emulator: tag + digest documents per [`architecture-backends.md`](../architecture-backends.md)
- [ ] Transactional batch manifest write on same partition
- [ ] Referrers via `subjectDigest` query or index

### Implement

- [ ] `CosmosRegistryMetadata`
- [ ] Parity with DynamoDB read-optimized duplication (gzip payload)

---

## 2.8 — Azure edge (`oci-registry-azure`)

Validate on [§2.0](#20--local-azure-dev-environment-azurite-cosmos-emulator-functions) before deployed Azure.

### Test first

- [ ] `func host start` or direct bin: `GET /v2/` same as local
- [ ] APIM JWT validation (or Functions Easy Auth) — same semantics as AWS Cognito JWT at edge

### Implement

- [ ] `src/bin/azure.rs` + infra per [`architecture-edge.md`](../architecture-edge.md) Azure section
- [ ] [`architecture-auth.md`](../architecture-auth.md) Azure notes

---

## 2.9 — Conformance expansion (Phase 2 deliverables)

Policy and profile C details: [`guidelines-conformance.md`](../guidelines-conformance.md).

### Test first

- [ ] **Profile C** ([§2.0](#20--local-azure-dev-environment-azurite-cosmos-emulator-functions)): full suite on `oci-registry-azure` + Azurite + Cosmos
- [ ] Profiles A + B still green after Phase 2 handler changes
- [ ] Conformance tests for mount + referrers enabled (if in suite)
- [ ] Phase 2 upload presign tests in conformance or custom integration suite

### Implement

- [ ] `dev/run-conformance-azure.sh` wired in CI (`conformance-azure` job) per [`guidelines-conformance.md`](../guidelines-conformance.md)

---

## README — Phase 2

- [ ] README: **Local Azure** stack (§2.0), `oci-registry-azure`, optional `func host start`, env vars (`AZURE_STORAGE_CONNECTION_STRING`, `COSMOS_*`, `OCI_*`)
- [ ] README: conformance profile C, presigned upload threshold env (`PRESIGN_UPLOAD_MIN_BYTES` if added), mount + referrers API notes for operators
- [ ] README binaries table includes `oci-registry-azure` + `azure` feature
- [ ] Each new §2.x script, compose file, or config knob documented in crate README same PR

---

## Phase 2 — Done checklist

- [ ] All sections above checked
- [ ] README checklist (above) complete
- [ ] [`plan-3.md`](./plan-3.md) prerequisites met
