# oci-registry — implementation plan

Phased delivery checklist for the OCI Distribution registry (`catalog/oci-registry`). **Normative rules** live in [`guidelines.md`](../guidelines.md) and [`guidelines-conformance.md`](../guidelines-conformance.md); **system design** in [`architecture.md`](../architecture.md) and linked `architecture-*.md` docs. This folder is the **schedule and task list** only — do not duplicate architecture prose here.

---

## Phase map

| Phase | Doc | Outcome | Conformance profile |
|-------|-----|---------|---------------------|
| **0** | [phase-0-scaffolding.md](./phase-0-scaffolding.md) | Workspace crate, traits, mocks, HTTP shell, `dev/` layout | — |
| **1** | [phase-1-local.md](./phase-1-local.md) | `oci-registry-local` (FS + SQLite), read + push paths, profile A green | **A** (`:5000`) |
| **2** | [phase-2-aws.md](./phase-2-aws.md) | S3 + DynamoDB, emulated stack, SAM, optional deployed smoke | **B** (`:5001`) |
| **3** | [phase-3-azure.md](./phase-3-azure.md) | Blob + Cosmos, Azurite + emulator, optional deployed smoke | **C** (`:5002`) |
| **4** | [phase-4-operations.md](./phase-4-operations.md) | DELETE APIs, GC job, observability, live-cloud smoke | A/B/C + `OCI_TEST_CONTENT_MANAGEMENT=1` |

**Deferred (not in these phases):** GCP metadata backend — see [architecture-backends.md](../architecture-backends.md#gcp-deferred).

```text
Phase 0 ──► Phase 1 (local) ──► Phase 2 (AWS) ──► Phase 3 (Azure)
                                      │                  │
                                      └────────┬─────────┘
                                               ▼
                                         Phase 4 (ops)
```

Phases **2** and **3** may proceed in parallel once **1** exit criteria are met (shared `lib` handlers; cloud binaries wire traits only).

---

## Target crate layout (all phases)

Single workspace member under `catalog/oci-registry/` ([architecture-overview.md](../architecture-overview.md)):

```text
catalog/oci-registry/
  Cargo.toml                    # [lib] + [[bin]] × 3; features: local | aws | azure
  README.md                     # how to run each binary / profile (updated per guidelines)
  src/
    lib.rs                      # Router, handlers, RegistryError, authZ
    traits.rs                   # BlobStore, RegistryMetadata (+ types)
    auth.rs                     # Claim parse; dev bypass gate
    observability.rs            # oci_tag_download, metrics hooks
    handlers/                   # /v2/* — trait-only dependencies
    storage/
      mock.rs                   # test doubles (always built for tests)
      local.rs                  # feature = "local"
      aws.rs                    # feature = "aws"
      azure.rs                  # feature = "azure"
    bin/
      local.rs                  # oci-registry-local
      aws.rs                    # oci-registry-aws
      azure.rs                  # oci-registry-azure
  dev/
    docker-compose.aws.yml      # MinIO + DynamoDB Local
    docker-compose.azure.yml    # Azurite + Cosmos emulator
    bootstrap-aws.sh
    bootstrap-azure.sh
    template.yaml               # SAM → oci-registry-aws
    host.json + func/           # Azure Functions host (azure phase)
    mint-test-jwt.sh            # infraweave_oci::… claims for local/CI
    run-conformance-local.sh
    run-conformance-aws.sh
    run-conformance-azure.sh
```

**Rules:** one cloud feature per binary artifact; handlers never import `aws_sdk_*` / `azure_*` directly — only trait impls in `src/storage/*`.

---

## Scaffolding examples by environment

These are **starting trees** for Phase 0 work units; flesh out in phase docs.

### Local (`oci-registry-local`)

```text
# Run (profile A)
cargo run --bin oci-registry-local --features local
# Env (illustrative — document exact names in crate README)
OCI_REGISTRY_DATA_DIR=./.data/oci-registry
OCI_REGISTRY_LISTEN=127.0.0.1:5000
REGISTRY_PUBLIC_URL=http://127.0.0.1:5000
OCI_AUTH_BYPASS=0   # prefer mint-test-jwt.sh for CI parity
```

Backends: **FS** blob keys `v2/blobs/sha256/…/data`; **SQLite** single-file metadata mirroring DynamoDB PK/SK semantics ([architecture-backends.md](../architecture-backends.md#local-dev--profile-a)).

### AWS emulated (profile B)

```text
docker compose -f dev/docker-compose.aws.yml up -d
./dev/bootstrap-aws.sh          # bucket, table, MinIO creds
sam build && sam local start-api -p 5001
# OCI_ROOT_URL=http://127.0.0.1:5001
```

Backends: **MinIO** (S3 API) + **DynamoDB Local**; same `S3BlobStore` / `DynamoRegistryMetadata` as production ([phase-2-aws.md](./phase-2-aws.md)).

### Azure emulated (profile C)

```text
docker compose -f dev/docker-compose.azure.yml up -d
./dev/bootstrap-azure.sh
cargo run --bin oci-registry-azure --features azure
# listen :5002 — OCI_ROOT_URL=http://127.0.0.1:5002
```

Backends: **Azurite** + **Cosmos DB emulator** ([phase-3-azure.md](./phase-3-azure.md)).

### Deployed smoke (post-merge / nightly — not PR gates)

| Cloud | Edge | Compute | Data |
|-------|------|---------|------|
| AWS | `registry.example.com` → API GW HTTP API + Cognito JWT | Lambda `oci-registry-aws` | S3 + DynamoDB |
| Azure | Custom domain → APIM + JWT | Function `oci-registry-azure` | Storage account + Cosmos |

See [architecture-edge.md](../architecture-edge.md) and per-phase deploy work units.

---

## Endpoint delivery order (cross-phase)

Handlers are built once in `lib`; phases enable **storage + dev stack** per cloud. Suggested HTTP rollout inside Phase 1 (local proves behavior first):

| Order | Endpoint group | Architecture |
|-------|----------------|--------------|
| 1 | `GET /v2/` capability | [flows — capability](./architecture-flows.md#capability-check) |
| 2 | Manifest GET (tag + digest) | [pull manifest](./architecture-flows.md#pull-manifest-by-tag-tag-download) |
| 3 | Blob GET/HEAD → **307** | [pull blob](./architecture-flows.md#pull-blob-layer) |
| 4 | `tags/list`, referrers | [list tags](./architecture-flows.md#list-tags), [referrers](./architecture-flows.md#referrers-oci-v11) |
| 5 | Upload POST/PATCH/PUT | [push blob](./architecture-flows.md#push-blob) |
| 6 | Manifest PUT | [push manifest](./architecture-flows.md#push-manifest--tag) |
| 7 | Blob mount | [blob mount](./architecture-flows.md#blob-mount) |
| 8 | DELETE manifest/blob | [end-9, end-10](./architecture-flows.md#delete-manifest-end-9) — **Phase 4** |

Full matrix: [endpoint → trait matrix](./architecture-flows.md#endpoint--trait-matrix).

---

## CI pyramid (when to turn on jobs)

| Gate | After phase | Job name (suggested) |
|------|-------------|----------------------|
| `cargo test -p oci-registry` + HTTP mocks | 0 | `oci-registry-unit` |
| Conformance profile **A** | 1 exit | `conformance-local` |
| Profile **B** | 2 stable emulators | `conformance-aws` |
| Profile **C** | 3 stable emulators | `conformance-azure` |
| `OCI_TEST_CONTENT_MANAGEMENT=1` | 4 DELETE done | extend A/B/C |
| Deployed AWS/Azure | 2/3 smoke work units | nightly only |

Detail: [guidelines-conformance.md](../guidelines-conformance.md#ci-pyramid).

---

## Work unit convention

Every checkbox phase doc uses the skeleton from [guidelines.md — Plan task shape](../guidelines.md#plan-task-shape):

- Phase-level **Prerequisites** / **Exit criteria**
- Work units `N.M` with **Spec**, **Architecture**, **Test layer**
- **Test first** → **Implement** → **README** (when run surface changes)

Infrastructure-only units may use **Setup** / **Verify** instead of Test first / Implement.

---

## Related docs

| Doc | Role |
|-----|------|
| [`../architecture.md`](../architecture.md) | Architecture index |
| [`../guidelines.md`](../guidelines.md) | TDD, traits-only handlers, anti-patterns |
| [`../guidelines-conformance.md`](../guidelines-conformance.md) | Profiles A/B/C, env vars, CI jobs |
| [`../../../oci-registry/README.md`](../../../oci-registry/README.md) | Runnable commands (create/update with implementation) |
