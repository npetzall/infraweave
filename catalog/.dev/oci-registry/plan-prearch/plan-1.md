# Phase 1 — AWS MVP

**Parent**: [`plan.md`](./plan.md) · **Prerequisite**: [`plan-0.md`](./plan-0.md) complete · **Architecture**: [`architecture-flows.md`](../architecture-flows.md), [`architecture-edge.md`](../architecture-edge.md), [`architecture-observability.md`](../architecture-observability.md), [`architecture-backends.md`](../architecture-backends.md)

## Goal

Ship a working OCI registry on **AWS**: S3 + DynamoDB single-table, Lambda + dedicated API Gateway, Cognito JWT at edge, repo authZ in Lambda. Clients pull blobs via **307** + presigned GET; push via registry-hosted upload sessions (Phase 1 — no upload redirect). Include **tag download logging**.

## Exit criteria

- [ ] **Conformance profile A**: distribution-spec suite green vs `oci-registry-local` ([`guidelines-conformance.md`](../guidelines-conformance.md))
- [ ] **Conformance profile B**: same suite green vs `oci-registry-aws` on SAM local + MinIO + DynamoDB Local (§1.0, [`guidelines-conformance.md`](../guidelines-conformance.md))
- [ ] `oci-registry-aws` deployable; smoke test: push/pull small image with pre-provisioned JWT
- [ ] `GET` blob returns **307** + `Docker-Content-Digest`; client fetches bytes from S3 without `Authorization`
- [ ] Tag manifest GET emits `oci_tag_download` structured log ([`architecture-observability.md`](../architecture-observability.md))
- [ ] DELETE manifest/blob returns **405** if not implemented (acceptable MVP)
- [ ] Local AWS stack (§1.0): DynamoDB Local + MinIO + **SAM CLI** can run `oci-registry-aws` and pass storage + HTTP smoke tests

---

## 1.0 — Local AWS dev environment (DynamoDB Local, MinIO, SAM CLI)

Use this stack to develop and test the **`aws`** feature against real SDK code paths (S3 + DynamoDB + Lambda handler) without deploying to AWS.

**Conformance** profiles A and B: see [`guidelines-conformance.md`](../guidelines-conformance.md). §1.0 below is the emulated AWS stack for profile B.

| Component | Image / tool | Role |
|-----------|----------------|------|
| **DynamoDB Local** | `amazon/dynamodb-local` (container) | `RegistryMetadata` — table `oci-registry-table`, GSI scaffold |
| **MinIO** | `minio/minio` (container) | S3-compatible `BlobStore` — bucket for `v2/blobs/…` |
| **AWS SAM CLI** | `sam` on host | `sam build` / `sam local start-api` or `sam local invoke` for `oci-registry-aws` |

All assets live under **`catalog/oci-registry/dev/`** — self-contained; do **not** import or depend on the repo `integration-tests` crate or its scaffold.

### Setup (document + automate)

- [ ] Add `catalog/oci-registry/dev/docker-compose.aws.yml`: services `dynamodb` (`amazon/dynamodb-local`, port **8000**), `minio` (`minio/minio`, API **9000**); shared Docker network
- [ ] Bootstrap script or `make dev-aws-up`: create S3 bucket on MinIO; `CreateTable` for `oci-registry-table` + `GSI_Referrers` per [`architecture-backends.md`](../architecture-backends.md)
- [ ] Document required env for local AWS (host → containers):

  | Variable | Example | Purpose |
  |----------|---------|---------|
  | `AWS_REGION` | `us-west-2` | SDK region |
  | `AWS_ACCESS_KEY_ID` / `AWS_SECRET_ACCESS_KEY` | `minio` / `minio123` | MinIO credentials |
  | `AWS_ENDPOINT_URL_S3` | `http://127.0.0.1:9000` | S3 API → MinIO |
  | `DYNAMODB_ENDPOINT` or `AWS_ENDPOINT_URL_DYNAMODB` | `http://127.0.0.1:8000` | DynamoDB API → DynamoDB Local |
  | `OCI_S3_BUCKET` | `oci-registry` | Blob bucket name |
  | `OCI_DYNAMODB_TABLE` | `oci-registry-table` | Metadata table |
  | `OCI_AUTH_BYPASS` | `1` (dev only) | Skip JWT in-process tests; SAM may still pass synthetic claims |

- [ ] Add **`template.yaml`** (SAM): `AWS::Serverless::Function` for `oci-registry-aws`, `HttpApi` or `Api` event `ANY /v2/{proxy+}`, env vars pointing at host endpoints above (`host.docker.internal` from SAM emulator to MinIO/DynamoDB on host, or attach Lambda container to compose network)
- [ ] README + `dev/README.md`: [AWS SAM CLI](https://docs.aws.amazon.com/serverless-application-model/latest/developerguide/install-sam-cli.html) install and `sam --version`

### Test first (against local AWS stack)

- [ ] With compose up: Rust **integration** tests (`aws` feature) hit MinIO + DynamoDB Local — `put_manifest` / `put` blob / `presign_get` round-trip
- [ ] `sam build` produces artifact for `oci-registry-aws`
- [ ] `sam local start-api` (or `sam local invoke` + sample API GW v2 event): `GET /v2/` → **200**; `GET …/blobs/{digest}` → **307** to MinIO presigned URL
- [ ] `crane` / `go-containerregistry` push/pull against SAM local base URL + test JWT or bypass

### Distribution conformance (`aws` feature, local)

Per [`guidelines-conformance.md`](../guidelines-conformance.md) profile B — wire `dev/run-conformance-aws.sh` and document port **5001** (profile A uses **5000**).

- [ ] Conformance green with **307** blob pulls to MinIO (validates presigned GET on AWS code path)
- [ ] Record any tests skipped when `OCI_TEST_CONTENT_MANAGEMENT=0` (DELETE deferred to Phase 3)

### Implement

- [ ] `catalog/oci-registry/dev/docker-compose.aws.yml` + `bootstrap-aws.sh` (or `Makefile` in crate root: `dev-aws-up`, `dev-aws-down`)
- [ ] `catalog/oci-registry/dev/template.yaml` + `samconfig.toml` for `sam local start-api --env-vars dev/env.aws.json`
- [ ] `catalog/oci-registry/tests/aws_local.rs` (behind `aws` feature): either script-driven (`dev-aws-up` + env) **or** optional `testcontainers` dev-dependency **declared only in oci-registry** — not via `integration-tests`
- [ ] **`catalog/oci-registry/README.md`**: **Local AWS**, **Conformance** (profiles A + B), env var tables, ports :5000/:5001, `dev/run-conformance-*.sh` commands
- [ ] `dev/README.md`: prerequisites (Docker, SAM, Go), compose/SAM troubleshooting (crate README links here)
- [ ] `catalog/oci-registry/dev/run-conformance-aws.sh`: compose → bootstrap → `sam local start-api` → Go conformance → teardown
- [ ] `catalog/oci-registry/dev/run-conformance-local.sh`: `oci-registry-local` → conformance (profile A)

### Notes

- **MinIO** validates presigned GET/PUT signatures and **307** redirect behavior better than FS-only `oci-registry-local`.
- **DynamoDB Local** does not support every DynamoDB feature (e.g. some TTL/transact edge cases); document gaps; use real DynamoDB in nightly smoke if needed.
- **SAM CLI** emulates API Gateway + Lambda locally; production edge (Cognito JWT authorizer) is still validated in §1.13 — pass JWT claims in SAM event templates for authZ unit tests.

---

## 1.1 — HTTP router skeleton (`oci-registry-local`)

### Test first

- [ ] `GET /v2/` without auth (test bypass) → **200** `{}`
- [ ] Unknown path → **404** OCI error JSON
- [ ] `GET /v2/acme/widgets/manifests/latest` → repository name `acme/widgets` (multi-segment `<name>`) — [`architecture-edge.md#repository-path-name`](../architecture-edge.md#repository-path-name)
- [ ] Invalid repository name → **400** `NAME_INVALID`
- [ ] Router injects mock `BlobStore` + `RegistryMetadata` via `State`

### Implement

- [ ] Path parser: `/v2/{proxy+}` → `<name>` + operation suffix per [`architecture-edge.md#repository-path-name`](../architecture-edge.md#repository-path-name)
- [ ] Axum app in `oci-registry` with route module per resource
- [ ] `oci-registry-local` bin (same crate, `local` feature): wire `LocalFsBlobStore` + SQLite metadata
- [ ] Shared `AppState { blob, meta, base_url, auth }` builder

---

## 1.2 — AuthZ helpers (Lambda + local test JWT)

### Test first

- [ ] JWT claims `infraweave_oci::acme/app::r` allows GET manifest for `acme/app`
- [ ] Missing claim → **403**
- [ ] `::rw` required for `POST` upload and `PUT` manifest
- [ ] Test mode: env `OCI_AUTH_BYPASS=1` only in tests

### Implement

- [ ] Parse API GW v2 JWT claims / local Bearer decode
- [ ] `authorize(repo, method, claims) -> Result<()>`
- [ ] Document test token minting in [`architecture-auth.md`](../architecture-auth.md)

---

## 1.3 — Registry base URL

### Test first

- [ ] Upload `POST` **202** `Location` uses `https://registry.example.com/v2/...` when `Host` + `X-Forwarded-Proto` set in request extensions
- [ ] Without forwarded headers, fall back to configured default base URL

### Implement

- [ ] `registry_base_url(headers, config) -> Url`
- [ ] Use in all `Location` response headers (upload session, manifest **201**)

---

## 1.4 — `GET /v2/` (end-1)

### Test first

- [ ] Authenticated `GET /v2/` → **200**, `Docker-Distribution-API-Version: registry/2.0` (if required by conformance)

### Implement

- [ ] Handler; edge JWT already validated

---

## 1.5 — Manifest GET/HEAD (end-3) + tag observability

### Test first

- [ ] `GET …/manifests/latest` → **200**, body matches stored gzip payload decompressed, `Docker-Content-Digest` = hash of **raw** JSON ([`architecture-backends.md`](../architecture-backends.md))
- [ ] `GET …/manifests/sha256:…` resolves digest row
- [ ] Unknown tag → **404** `MANIFEST_UNKNOWN`
- [ ] Tag GET: capture log output contains `event=oci_tag_download`, `repository`, `tag`, `digest` (no manifest body, no JWT) — [`architecture-observability.md`](../architecture-observability.md)
- [ ] Digest-only GET does **not** emit `oci_tag_download` (optional `oci_manifest_download` later)

### Implement

- [ ] Handler calls `RegistryMetadata::get_manifest`
- [ ] `tracing::info!(…)` on successful tag resolution
- [ ] HEAD manifest returns headers without body

---

## 1.6 — Manifest PUT (end-7)

### Test first

- [ ] `PUT …/manifests/my-tag` with valid OCI manifest → **201**, `Docker-Content-Digest` header
- [ ] Referenced layer digest missing in `BlobStore` → **400** `BLOB_UNKNOWN`
- [ ] Invalid JSON / wrong media type → **400**
- [ ] Gzip `ManifestPayload` over item budget → **413** (or OCI size error); budget per [`architecture-backends.md#manifest-payload-size-limit`](../architecture-backends.md#manifest-payload-size-limit)

### Implement

- [ ] Validate manifest schema (minimal v2 / OCI index rules)
- [ ] `BlobStore::head` each layer reference
- [ ] Enforce `len(gzip) ≤ OCI_MAX_MANIFEST_ITEM_BYTES − reserved_overhead(row)` for each transact row ([`architecture-backends.md#manifest-payload-size-limit`](../architecture-backends.md#manifest-payload-size-limit))
- [ ] `RegistryMetadata::put_manifest` atomic write (local: transaction; AWS: TransactWriteItems in §1.8)
- [ ] Store gzip `ManifestPayload`; set `TargetDigest` from raw bytes

---

## 1.7 — Tags list (end-8)

### Test first

- [ ] `GET …/tags/list` → tags in **lexical** order
- [ ] `?n=2&last=…` pagination stable ([`architecture-traits.md`](../architecture-traits.md))

### Implement

- [ ] `RegistryMetadata::list_tags`

---

## 1.8 — AWS `RegistryMetadata` (DynamoDB)

Uses **DynamoDB Local** from [§1.0](#10--local-aws-dev-environment-dynamodb-local-minio-sam-cli) (compose or testcontainers).

### Test first

- [ ] Against DynamoDB Local: `put_manifest` writes `TAG#` + `DIGEST#` rows per [`architecture-backends.md`](../architecture-backends.md)
- [ ] `get_manifest` by tag single `GetItem` returns payload + `TargetDigest`
- [ ] Upload row `UPLOAD#{uuid}` created and updated

### Implement

- [ ] Table `oci-registry-table`; PK `REPO#…`, SK patterns
- [ ] `TransactWriteItems` on manifest put
- [ ] GSI_Referrers table/index created (keys only OK if referrers API deferred to Phase 2)

---

## 1.9 — AWS `BlobStore` (S3)

Uses **MinIO** from [§1.0](#10--local-aws-dev-environment-dynamodb-local-minio-sam-cli) (S3-compatible endpoint).

### Test first

- [ ] Against MinIO: `put`/`head` at `v2/blobs/sha256/…/data`
- [ ] `presign_get` URL returns **200** when fetched without `Authorization`
- [ ] Digest mismatch on `put` does not leave orphan object (or documents cleanup)

### Implement

- [ ] `S3BlobStore` in `oci-registry` behind `cfg(feature = "aws")` (compiled into `oci-registry-aws` bin only)
- [ ] Presign TTL configurable (15–60+ min)

---

## 1.10 — Blob pull GET/HEAD (end-2) — **307**

### Test first

- [ ] `GET …/blobs/sha256:…` → **307**, `Location` presigned, `Docker-Content-Digest` set
- [ ] Follow redirect in integration test without `Authorization` header → **200** + correct bytes
- [ ] Missing blob → **404** `BLOB_UNKNOWN`
- [ ] `HEAD` blob: choose **200** from registry with digest header OR **307** — document choice; test matches impl ([`architecture-flows.md`](../architecture-flows.md#pull-blob-layer))

### Implement

- [ ] Handler: optional `head`, then `presign_get`
- [ ] Never stream large body through Lambda

---

## 1.11 — Blob push: POST / PATCH / PUT (end-4, 5, 6, 13)

### Test first

- [ ] `POST …/blobs/uploads/` → **202**, `Location` registry URL, `Range: 0-0`
- [ ] `PATCH` with `Content-Range` appends; wrong range → **416**
- [ ] `PUT …?digest=` → **201**, blob in `BlobStore`, `link_blob_to_repo` called
- [ ] `GET …/uploads/{uuid}` → **204** with `Range` header (end-13)
- [ ] Monolithic `POST ?digest=` under size cap → **201**
- [ ] Over API GW limit → **202** session fallback ([`architecture-flows.md#chunked-upload-end-4-5-6-13`](../architecture-flows.md#chunked-upload-end-4-5-6-13))

### Implement

- [ ] Upload orchestration using traits only (Phase 1 — no upload redirect)
- [ ] Session state in `RegistryMetadata`; bytes via `BlobStore::put` / partial writes
- [ ] Honor `OCI-Chunk-Min-Length` on **202** when client sends header

---

## 1.12 — Lambda binary (`oci-registry-aws`)

Validate in-process first, then via **SAM CLI** + [§1.0](#10--local-aws-dev-environment-dynamodb-local-minio-sam-cli) stack.

### Test first

- [ ] Lambda handler unit test: synthetic API GW event → same status as Axum local for `GET /v2/`
- [ ] `cargo build -p oci-registry --bin oci-registry-aws --no-default-features --features aws` succeeds
- [ ] `sam local invoke` with `dev/events/get-v2.json` → **200**
- [ ] `sam local start-api` against MinIO + DynamoDB Local (§1.0): blob **307** + manifest GET smoke

### Implement

- [ ] `src/bin/aws.rs`: `lambda_http` adapter wrapping shared router from `lib`
- [ ] Reuse `Arc` trait impls / AWS SDK clients across invocations ([`architecture-edge.md#compute--lambda--functions`](../architecture-edge.md#compute--lambda--functions))
- [ ] Env: `OCI_S3_BUCKET`, `OCI_DYNAMODB_TABLE`, optional Cognito issuer (for claim pass-through); SDK uses §1.0 endpoints when set
- [ ] `dev/template.yaml` + sample events under `dev/events/` for SAM

---

## 1.13 — Edge infrastructure (Terraform / CDK — project convention)

### Test first

- [ ] Manual or automated smoke: request without JWT → **401** from API GW
- [ ] Request with valid Cognito JWT + claim → reaches Lambda

### Implement

- [ ] Separate HTTP API `oci-registry-api` + custom domain — see [`architecture-edge.md`](../architecture-edge.md)
- [ ] Cognito JWT authorizer on `/v2/*` — [`architecture-auth.md`](../architecture-auth.md)
- [ ] Lambda IAM: S3 read/write, DynamoDB, logs

---

## 1.14 — Conformance (Phase 1 deliverables)

Policy, profiles, env vars, and CI jobs: [`guidelines-conformance.md`](../guidelines-conformance.md). This section is **what to wire** for Phase 1 exit.

### Test first

- [ ] Profile A: suite passes against `oci-registry-local`
- [ ] Profile B: suite passes against SAM local + §1.0 stack (`aws` feature)
- [ ] Same test case failures on A vs B documented (if any) — storage-only gaps
- [ ] `go-containerregistry` or `crane` push/pull against Profile A and Profile B URLs
- [ ] Redirect: conformance or integration asserts client does not send `Authorization` to MinIO presigned URL

### Implement

- [ ] `catalog/oci-registry/dev/run-conformance-local.sh` and `run-conformance-aws.sh` (paths relative to crate root)
- [ ] CI jobs `conformance-local` and `conformance-aws` per [`guidelines-conformance.md`](../guidelines-conformance.md)
- [ ] README: prerequisites (Go, Docker, SAM CLI), how to run conformance A only vs A+B; link `dev/` scripts by name

---

## README — Phase 1

Update **`catalog/oci-registry/README.md`** whenever §1.0–1.14 adds runnable/configurable items:

- [ ] **Running locally**: `oci-registry-local`, env vars (`OCI_AUTH_BYPASS`, test JWT pointer to [`architecture-auth.md`](../architecture-auth.md))
- [ ] **Local AWS**: `docker compose -f dev/docker-compose.aws.yml`, `bootstrap-aws.sh`, SAM `template.yaml`, `sam local start-api`
- [ ] **Conformance**: `./dev/run-conformance-local.sh`, `./dev/run-conformance-aws.sh`, `OCI_ROOT_URL` / `OCI_TEST_*`
- [ ] **Binaries table**: `oci-registry-local` | `oci-registry-aws` × features × default ports
- [ ] **`OCI_MAX_MANIFEST_ITEM_BYTES`**: default, formula `max_gzip = budget − reserved_overhead`, compressed-payload limit ([`architecture-backends.md#manifest-payload-size-limit`](../architecture-backends.md#manifest-payload-size-limit))
- [ ] New env var or script from any §1.x subsection reflected in README same PR

---

## Phase 1 — Done checklist

- [ ] All sections above checked
- [ ] README checklist (above) complete
- [ ] [`plan-2.md`](./plan-2.md) prerequisites met
- [ ] Architecture phase 1 scope in [`architecture.md`](../architecture.md) satisfied
