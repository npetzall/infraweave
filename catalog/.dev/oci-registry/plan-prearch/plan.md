# oci-registry — implementation plan (index)

OCI **Distribution Spec v1.1.1** registry: content-addressable blob store + indexed repository metadata on **AWS** (Lambda + API Gateway + S3 + DynamoDB) or **Azure** (Functions + APIM + Blob + Cosmos). **GCP metadata backend is Phase 4**.

**Design** lives in [`architecture.md`](../architecture.md) and linked `architecture-*.md` docs. **Execution** is split into phase plans below — each follows **TDD** (failing test → minimal implementation → refactor) with checkbox checklists.

---

## How to use these plans

1. Complete phases **in order** (0 → 4). Each phase lists **prerequisites** and **exit criteria**.
2. Within a phase, work **top to bottom** unless a dependency note says otherwise.
3. Follow [`guidelines.md`](../guidelines.md) (TDD, scope, patterns, [plan task shape](../guidelines.md#plan-task-shape)) for every work unit.
4. Follow [`guidelines-conformance.md`](../guidelines-conformance.md) for test layers and conformance profiles; deployed cloud is smoke only.
5. Crate at `catalog/oci-registry/` (workspace member); all dev tooling under `catalog/oci-registry/dev/` — **no** dependency on `integration-tests`.
6. Whenever you add something **runnable or configurable**, update [`catalog/oci-registry/README.md`](../../oci-registry/README.md) in the same PR (each phase plan has a README checklist).

Test layers and conformance profiles: [`guidelines-conformance.md`](../guidelines-conformance.md).

---

## Phased delivery

This index defines **phase scope** and **endpoint ownership** per phase:

| Phase | Plan | Endpoints (new in phase) |
|-------|------|--------------------------|
| **0** | [`plan-0.md`](./plan-0.md) | — (library only) |
| **1** | [`plan-1.md`](./plan-1.md) | end-1 … end-8, end-13; DELETE optional **405** |
| **2** | [`plan-2.md`](./plan-2.md) | end-11, end-12; upload presign paths |
| **3** | [`plan-3.md`](./plan-3.md) | end-9, end-10 |
| **4** | [`plan-4.md`](./plan-4.md) | Same as 1–3 on GCS + TBD metadata |

---

## Scope

System boundaries (in/out of scope), crate layout, and separation from `registry-core`: [`architecture-overview.md#system-boundaries`](../architecture-overview.md#system-boundaries).

Implementation principles and TDD: [`guidelines.md`](../guidelines.md).

---

## API surface (v1.1.1)

Full endpoint → phase mapping and per-phase TDD tasks are in the phase plans. Spec: [endpoints table](https://github.com/opencontainers/distribution-spec/blob/v1.1.1/spec.md#endpoints). Matrix: [`architecture-flows.md`](../architecture-flows.md#endpoint--trait-matrix).

| ID | Method | Path | Phase |
|----|--------|------|-------|
| end-1 | `GET` | `/v2/` | 1 |
| end-2 | `GET`/`HEAD` | `/v2/{name}/blobs/{digest}` | 1 |
| end-3 | `GET`/`HEAD` | `/v2/{name}/manifests/{ref}` | 1 |
| end-4a–c | `POST` | `/v2/{name}/blobs/uploads/` | 1 (mount: 2) |
| end-5 | `PATCH` | `…/uploads/{uuid}` | 1 |
| end-6 | `PUT` | `…/uploads/{uuid}?digest=` | 1 |
| end-7 | `PUT` | `/v2/{name}/manifests/{ref}` | 1 |
| end-8 | `GET` | `/v2/{name}/tags/list` | 1 |
| end-9–10 | `DELETE` | manifests / blobs | 3 |
| end-11 | `POST` | mount `?mount=&from=` | 2 |
| end-12 | `GET` | `/v2/{name}/referrers/{digest}` | 2 |
| end-13 | `GET` | `/v2/{name}/blobs/uploads/{uuid}` | 1 |

Spec vs OpenAPI vs storage model: [`architecture-http.md`](../architecture-http.md). Catalog context: [`../registry/registry_plan.md`](../registry/registry_plan.md).

---

## Crate README

**File**: `catalog/oci-registry/README.md` (crate root).

Update it whenever a phase introduces or changes runnable/configurable surface — not only at phase end. Typical sections (grow over phases):

| Section | Introduced in |
|---------|----------------|
| Layout, workspace, `cargo build` / `cargo test` | Phase 0 |
| Binaries × features, env vars, `oci-registry-local` | Phase 0–1 |
| Local AWS (compose, SAM, conformance B), ports :5000/:5001 | Phase 1 |
| Local Azure (compose, conformance C), port :5002 | Phase 2 |
| GC, DELETE, metrics / ops | Phase 3 |
| GCP local (Phase 4 metadata backend chosen) | Phase 4 |

Long emulator detail may live in `catalog/oci-registry/dev/README.md`; the crate README must still show **how to start** each workflow.

---

## Dependencies (workspace)

| Crate | Use |
|-------|-----|
| `axum` / `lambda_http` | HTTP |
| `aws-sdk-s3`, `aws-sdk-dynamodb` | `aws` feature |
| `azure_storage_blobs`, Cosmos SDK | `azure` feature |
| `sha2`, `serde_json` | Digests + manifests |
| `tracing` | Structured logs |
| `oci-spec` (evaluate) | Manifest/index types |

---

## Related docs

| File | Purpose |
|------|---------|
| [`guidelines.md`](../guidelines.md) | Implementation principles and engineering rules |
| [`architecture-http.md`](../architecture-http.md) | HTTP contract, spec vs OpenAPI |
| [`guidelines-conformance.md`](../guidelines-conformance.md) | Conformance profiles, CI pyramid, mock vs emulated vs live |
| [`plan-0.md`](./plan-0.md) … [`plan-4.md`](./plan-4.md) | Detailed TDD checklists per phase |
| [`architecture.md`](../architecture.md) | Architecture index |
| [`architecture-flows.md`](../architecture-flows.md#push-blob) | Upload sessions, chunked PATCH, presigned upload offload |
| [`architecture-auth.md`](../architecture-auth.md) | Edge + client auth |
| [`architecture-backends.md`](../architecture-backends.md) | DynamoDB / Cosmos layouts |
