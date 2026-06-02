# Phase 2 — AWS (emulated + deploy)

**Prerequisite**: [phase-1-local.md](./phase-1-local.md) exit criteria (shared `lib` handlers proven) · **Architecture**: [architecture-backends.md](../architecture-backends.md), [architecture-edge.md](../architecture-edge.md), [architecture-auth.md](../architecture-auth.md)

## Goal

Implement `S3BlobStore` and `DynamoRegistryMetadata`, run **conformance profile B** against MinIO + DynamoDB Local via SAM local (`:5001`), and add optional **deployed AWS smoke** (real S3, DynamoDB, API Gateway, Cognito) on nightly — without making cloud secrets a PR gate.

## Prerequisites

- [ ] Phase 1 exit (profile A green)
- [ ] Docker available for `dev/docker-compose.aws.yml`
- [ ] SAM CLI installed for local API emulation
- [ ] AWS SDK credentials for **emulators only** (static MinIO keys in compose)

## Exit criteria

- [ ] `./dev/bootstrap-aws.sh` creates bucket `oci-registry-blobs`, table `oci-registry-table`, GSI `GSI_Referrers`
- [ ] `sam local start-api` on `:5001` + profile B conformance green
- [ ] CI job `conformance-aws` enabled when team agrees (may trail Phase 2 merge by one PR)
- [ ] Deployed smoke documented (manual or nightly): JWT at API GW, **307** to real S3 presign
- [ ] Single artifact: `oci-registry-aws` only — never `aws`+`azure` features in one build

---

## 2.1 — AWS emulated stack (`dev/`)

**Architecture**: [guidelines-conformance.md#profile-b--aws](../guidelines-conformance.md), [architecture-overview.md](../architecture-overview.md)

**Test layer**: Setup / Verify

### Setup

- [ ] `dev/docker-compose.aws.yml`: MinIO (S3 API), DynamoDB Local
- [ ] `dev/bootstrap-aws.sh`: create bucket, enable versioning if required, create table + GSI per [architecture-backends.md](../architecture-backends.md)
- [ ] Document env: `AWS_ENDPOINT_URL`, `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, region

### Verify

- [ ] `docker compose up` + bootstrap exits 0; `aws dynamodb list-tables` (or awslocal) shows `oci-registry-table`

---

## 2.2 — `S3BlobStore`

**Architecture**: [architecture-backends.md#aws--blobstore-s3](../architecture-backends.md#aws--blobstore-s3), [architecture-traits.md](../architecture-traits.md)

**Test layer**: local trait tests vs MinIO (emulated)

### Test first

- [ ] `head` / `put` / `delete` against MinIO with key `v2/blobs/sha256/…/data`
- [ ] `presign_get` URL fetches object without `Authorization` header
- [ ] Presign TTL ≥ configured minimum (15+ minutes)

### Implement

- [ ] `src/storage/aws.rs`: `S3BlobStore` with `aws-sdk-s3`
- [ ] `cfg(feature = "aws")` only; no AWS types in handlers

---

## 2.3 — `DynamoRegistryMetadata`

**Architecture**: [architecture-backends.md#aws--registrymetadata-dynamodb-single-table](../architecture-backends.md#aws--registrymetadata-dynamodb-single-table)

**Test layer**: emulated (DynamoDB Local)

### Test first

- [ ] `put_manifest` transact writes `TAG#` + `DIGEST#`; tag pull one `GetItem`
- [ ] `Docker-Content-Digest` header uses `TargetDigest` from row (raw hash)
- [ ] Referrers: `SubjectDigest` on `DIGEST#` row queryable via `GSI_Referrers`
- [ ] Upload session row `UPLOAD#{uuid}` lifecycle

### Implement

- [ ] DynamoDB impl hiding PK/SK inside module
- [ ] Manifest gzip item budget **4096** B enforced before transact

---

## 2.4 — `oci-registry-aws` binary + Lambda adapter

**Architecture**: [architecture-http.md#compute-adapter](../architecture-http.md#compute-adapter), [architecture-edge.md](../architecture-edge.md)

**Test layer**: HTTP + mock → SAM local

### Test first

- [ ] Lambda handler forwards API GW v2 event to same `Router` as local
- [ ] `REGISTRY_PUBLIC_URL` or `Host` + `X-Forwarded-Proto` produces registry-hosted upload `Location` URLs

### Implement

- [ ] `src/bin/aws.rs`: Lambda runtime bootstrap
- [ ] Reuse `Arc` clients across warm starts (`OnceLock`)

---

## 2.5 — SAM template and local API

**Architecture**: [architecture-edge.md#aws](../architecture-edge.md#aws)

**Test layer**: Setup / Verify → conformance B

### Setup

- [ ] `dev/template.yaml`: Lambda `oci-registry-aws`, HTTP API routes `/v2/{proxy+}`
- [ ] Environment variables for bucket, table, endpoints (MinIO override in local)

### Verify

- [ ] `sam build && sam local start-api -p 5001` serves `/v2/`

### README (when runnable or configurable)

- [ ] Document profile B startup sequence in crate README + `dev/README.md`

---

## 2.6 — Presigned upload offload (optional slice)

**Architecture**: [architecture-flows.md#presigned-upload-offload](../architecture-flows.md#presigned-upload-offload), [guidelines.md#blob-upload-modes](../guidelines.md#blob-upload-modes)

**Test layer**: emulated (MinIO) — enable when large-blob push required

### Test first

- [ ] `POST` upload with size ≥ `PRESIGN_UPLOAD_MIN_BYTES` → **202** `Location` to MinIO host
- [ ] Client `PUT` to presigned URL, then registry `PUT ?digest=` on registry host → **201**
- [ ] Declared `Content-Length` on `POST` enforced at `presign_put` generation

### Implement

- [ ] `S3BlobStore::presign_put`
- [ ] Handler branch for presigned vs registry-hosted ([guidelines.md](../guidelines.md))

### Notes

Profile B may pass without presigned **PUT** if conformance blobs stay under API GW limits; implement before production large-layer push.

---

## 2.7 — Conformance profile B

**Architecture**: [guidelines-conformance.md](../guidelines-conformance.md)

**Test layer**: conformance (profile B)

### Test first

- [ ] `./dev/run-conformance-aws.sh` exits 0 at `OCI_ROOT_URL=http://127.0.0.1:5001`

### Implement

- [ ] Same env as profile A; JWT mint script against local authorizer bypass or test authorizer in SAM

---

## 2.8 — Edge: API Gateway + Cognito (deploy)

**Architecture**: [architecture-edge.md](../architecture-edge.md), [architecture-auth.md](../architecture-auth.md#cognito-jwt-authorizer-production)

**Test layer**: live cloud (nightly / manual only)

### Setup

- [ ] IaC or documented steps: HTTP API `oci-registry-api`, custom domain `registry.example.com`, JWT authorizer (issuer, audience)
- [ ] Separate from catalog `api.example.com` ([registry_apigw_routing.md](../../registry/registry_apigw_routing.md))
- [ ] Cognito pre-token Lambda adds `infraweave_oci::<repo>::r|rw` claims

### Verify

- [ ] `GET /v2/` with production token → **200**
- [ ] `crane` smoke (optional): pull one small image through registry host

### Notes

No `GET /token` distribution endpoint in MVP.

---

## 2.9 — Deployed conformance smoke (nightly)

**Test layer**: live cloud

### Test first

- [ ] Nightly workflow: deploy to dev account → conformance with real `OCI_ROOT_URL` + Cognito-minted token (secrets in CI vault)

### Implement

- [ ] Workflow `conformance-aws-deployed` — `if: schedule` or manual `workflow_dispatch`

### README (when runnable or configurable)

- [ ] Operator section: required secrets, rollback, presign CORS on S3 bucket if clients hit browser (not typical for tofu CLI)

---

## 2.10 — Observability (AWS)

**Architecture**: [architecture-observability.md](../architecture-observability.md)

**Test layer**: HTTP + mock → emulated smoke

### Test first

- [ ] Structured JSON logs include `oci_tag_download`, presign failures, error codes — never JWT or presigned URL in log line

### Implement

- [ ] CloudWatch log format via Lambda JSON logger; metric hooks for presign/GC (GC metrics in Phase 4)
