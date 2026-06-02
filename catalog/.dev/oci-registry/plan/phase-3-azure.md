# Phase 3 — Azure (emulated + deploy)

**Prerequisite**: [phase-1-local.md](./phase-1-local.md) exit criteria · **Architecture**: [architecture-backends.md](../architecture-backends.md), [architecture-edge.md](../architecture-edge.md)

## Goal

Implement `AzureBlobStore` and `CosmosRegistryMetadata`, run **conformance profile C** on Azurite + Cosmos emulator (`:5002`), and add optional **deployed Azure smoke** (APIM, Functions, real storage) on nightly. Handlers remain unchanged from Phase 1 — only `src/storage/azure.rs` and `oci-registry-azure` binary differ.

## Prerequisites

- [ ] Phase 1 exit (profile A green)
- [ ] Phase 2 not required for Azure work, but **recommended** so S3/Dynamo patterns are settled before Cosmos transact quirks
- [ ] Docker for `dev/docker-compose.azure.yml`
- [ ] Cosmos DB emulator license / container accepted in dev docs

## Exit criteria

- [ ] `./dev/bootstrap-azure.sh` creates storage container + Cosmos database/container with partition key design matching [architecture-backends.md](../architecture-backends.md) (Azure section)
- [ ] `cargo run --bin oci-registry-azure --features azure` on `:5002` + profile C conformance green
- [ ] CI job `conformance-azure` when emulator stable in CI runners
- [ ] Deployed smoke documented (APIM JWT, Function, real Blob SAS **307**)
- [ ] Artifact `oci-registry-azure` only — no combined `aws`+`azure` build

---

## 3.1 — Azure emulated stack (`dev/`)

**Architecture**: [guidelines-conformance.md#profile-c--azure](../guidelines-conformance.md)

**Test layer**: Setup / Verify

### Setup

- [ ] `dev/docker-compose.azure.yml`: Azurite (blob), Cosmos emulator
- [ ] `dev/bootstrap-azure.sh`: container `oci-registry-blobs`, Cosmos container with partition `/pk`
- [ ] Document connection strings / emulator endpoints

### Verify

- [ ] Compose healthy; bootstrap creates resources idempotently

---

## 3.2 — `AzureBlobStore`

**Architecture**: [architecture-backends.md](../architecture-backends.md) (Azure blob section)

**Test layer**: emulated (Azurite)

### Test first

- [ ] Blob path `v2/blobs/sha256/…/data` round-trip `put` / `head`
- [ ] `presign_get` returns SAS URL; GET without registry `Authorization` succeeds
- [ ] SAS TTL aligns with catalog presign policy (15–60+ min)

### Implement

- [ ] `src/storage/azure.rs`: `AzureBlobStore` using `azure_storage_blobs`
- [ ] `cfg(feature = "azure")` only

---

## 3.3 — `CosmosRegistryMetadata`

**Architecture**: [architecture-backends.md](../architecture-backends.md) (Cosmos shapes)

**Test layer**: emulated (Cosmos emulator)

### Test first

- [ ] Partition `REPO-{repo}` (or equivalent) holds `TAG-`, `DIGEST-`, `UPLOAD-` docs
- [ ] Transactional batch on same partition for `put_manifest`
- [ ] Manifest item budget **1024** B enforced ([architecture-backends.md#manifest-payload-size-limit](../architecture-backends.md#manifest-payload-size-limit))
- [ ] Referrers via `subjectDigest` query / index

### Implement

- [ ] Hide Cosmos document IDs and partition keys inside module
- [ ] `TargetDigest` / gzip rules identical to AWS semantics

---

## 3.4 — `oci-registry-azure` binary + Functions adapter

**Architecture**: [architecture-edge.md](../architecture-edge.md) (Azure section), [architecture-http.md](../architecture-http.md)

**Test layer**: HTTP + mock → emulated

### Test first

- [ ] Functions HTTP trigger maps to same `Router` as local/AWS
- [ ] Registry `Location` headers use forwarded host/proto or `REGISTRY_PUBLIC_URL`

### Implement

- [ ] `src/bin/azure.rs`
- [ ] `dev/host.json` + function project wiring for local Functions host (if used) OR standalone `cargo run` for profile C per [guidelines-conformance.md](../guidelines-conformance.md)

---

## 3.5 — Presigned upload offload (Azure)

**Architecture**: [architecture-flows.md#presigned-upload-offload](../architecture-flows.md#presigned-upload-offload)

**Test layer**: emulated — same work unit pattern as [phase-2-aws.md §2.6](./phase-2-aws.md#26--presigned-upload-offload-optional-slice)

### Test first

- [ ] `presign_put` SAS allows client PUT; terminal `PUT ?digest=` on registry host completes session

### Implement

- [ ] `AzureBlobStore::presign_put`

---

## 3.6 — Conformance profile C

**Architecture**: [guidelines-conformance.md](../guidelines-conformance.md)

**Test layer**: conformance (profile C)

### Test first

- [ ] `./dev/run-conformance-azure.sh` exits 0 at `OCI_ROOT_URL=http://127.0.0.1:5002`

### Implement

- [ ] Pin conformance v1.1.1; same auth env as profiles A/B

### README (when runnable or configurable)

- [ ] Crate README: profile C commands

---

## 3.7 — Edge: APIM + JWT (deploy)

**Architecture**: [architecture-edge.md](../architecture-edge.md), [architecture-auth.md](../architecture-auth.md)

**Test layer**: live cloud (nightly / manual)

### Setup

- [ ] APIM API + custom domain for registry host; JWT validation policy (Entra / B2C per product choice)
- [ ] Azure Function deployed with managed identity to Blob + Cosmos

### Verify

- [ ] `GET /v2/` with valid token → **200**
- [ ] Blob GET → **307** to `*.blob.core.windows.net` (or CDN) SAS

### Notes

Mirror AWS separation: registry host ≠ catalog API host.

---

## 3.8 — Deployed smoke (nightly)

**Test layer**: live cloud

### Test first

- [ ] Scheduled workflow: conformance against dev subscription URL + test principal

### Implement

- [ ] `conformance-azure-deployed` workflow; secrets in vault

---

## 3.9 — Observability (Azure)

**Architecture**: [architecture-observability.md](../architecture-observability.md)

**Test layer**: emulated → live smoke

### Test first

- [ ] Application Insights / Azure Monitor structured logs for `oci_tag_download`, errors — no secrets in properties

### Implement

- [ ] Align field names with AWS JSON logs for cross-cloud dashboards
