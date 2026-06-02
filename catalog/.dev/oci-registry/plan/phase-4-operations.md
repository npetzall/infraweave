# Phase 4 — Content management, GC, and operations

**Prerequisite**: [phase-1-local.md](./phase-1-local.md) exit criteria; [phase-2-aws.md](./phase-2-aws.md) and/or [phase-3-azure.md](./phase-3-azure.md) for cloud-specific GC deployment · **Architecture**: [architecture-flows.md](../architecture-flows.md) (DELETE), [architecture-operations.md](../architecture-operations.md), [architecture-observability.md](../architecture-observability.md)

## Goal

Implement spec-compliant **DELETE** for manifests and blobs, enable conformance **content management** (`OCI_TEST_CONTENT_MANAGEMENT=1`), ship a **scheduled GC worker** with dry-run, and complete operator runbook + metrics — validated on local first, then emulated/live cloud.

## Prerequisites

- [ ] Push/pull/referrers green on profile A (minimum)
- [ ] Global blob refcheck design understood ([architecture-flows.md#delete-blob-end-10](../architecture-flows.md#delete-blob-end-10))
- [ ] Product decision: GC cron schedule and dry-run default for first prod run

## Exit criteria

- [ ] `DELETE` manifest → **202**; blob DELETE respects global refcheck → **202** or **403**
- [ ] Profiles A/B/C conformance with `OCI_TEST_CONTENT_MANAGEMENT=1`
- [ ] GC job: dry-run lists candidates; live run deletes only unreferenced blobs ([architecture-operations.md](../architecture-operations.md))
- [ ] Operator runbook in `dev/README.md` or `docs_internal` pointer: GC races, dry-run, emergency stop
- [ ] Metrics: GC success/failure, presign errors ([architecture-observability.md](../architecture-observability.md))

---

## 4.1 — DELETE manifest (end-9)

**Spec**: [Delete Manifest](https://github.com/opencontainers/distribution-spec/blob/v1.1.1/spec.md#delete-manifest)
**Architecture**: [architecture-flows.md#delete-manifest-end-9](../architecture-flows.md#delete-manifest-end-9)
**Test layer**: HTTP + mock → local → conformance

### Test first

- [ ] `DELETE …/manifests/{tag}` with `::rw` → **202**; tag row gone; `DIGEST#` remains if other tags reference same digest
- [ ] `DELETE` by digest removes `DIGEST#` and associated `TAG#` rows atomically
- [ ] Unknown ref → **404** `MANIFEST_UNKNOWN`
- [ ] Missing `::rw` → **403**
- [ ] Blob bytes remain in **BS** after manifest delete

### Implement

- [ ] Handler → `rm.delete_manifest`; update referrer GSI/index edges
- [ ] Replace **405** stubs from Phase 1

---

## 4.2 — DELETE blob (end-10)

**Spec**: [Delete Blob](https://github.com/opencontainers/distribution-spec/blob/v1.1.1/spec.md#delete-blob)
**Architecture**: [architecture-flows.md#delete-blob-end-10](../architecture-flows.md#delete-blob-end-10), [architecture-traits.md](../architecture-traits.md) (GC-only `delete`)

**Test layer**: HTTP + mock → local → conformance

### Test first

- [ ] DELETE digest still referenced by any repo manifest → **403** (not **202**)
- [ ] DELETE unreferenced digest → **202**; object removed from **BS**
- [ ] Repo A and repo B share layer: DELETE from A does not remove bytes while B references digest

### Implement

- [ ] **RM** global refcheck (scan `References` on all `DIGEST#` rows or maintained refcount)
- [ ] **BS** `delete` only after refcheck passes

---

## 4.3 — Conformance content management

**Architecture**: [guidelines-conformance.md#content-management](../guidelines-conformance.md)

**Test layer**: conformance (A, then B/C)

### Test first

- [ ] `OCI_TEST_CONTENT_MANAGEMENT=1` in `run-conformance-*.sh` → suite green on profiles A/B/C

### Implement

- [ ] Fix DELETE edge cases surfaced by official suite
- [ ] CI: flip env var in conformance jobs after 4.1–4.2 pass locally

---

## 4.4 — GC worker (library + CLI)

**Architecture**: [architecture-operations.md](../architecture-operations.md)

**Test layer**: mock → local trait tests

### Test first

- [ ] Dry-run: candidate digest listed, `BlobStore::delete` not called
- [ ] Live: orphan digest deleted; referenced digest skipped even if unlinked from one repo
- [ ] Race: manifest re-push between scan and delete → refcheck before delete prevents wrongful removal

### Implement

- [ ] `oci-registry-gc` subcommand or separate bin behind `local`/`aws`/`azure` features
- [ ] `OCI_GC_DRY_RUN=1` support; structured log `event=oci_gc_dry_run`

### README (when runnable or configurable)

- [ ] Operator: first production run must be dry-run; sample log interpretation

---

## 4.5 — GC scheduled deployment (AWS)

**Architecture**: [architecture-operations.md#gc-job-design](../architecture-operations.md#gc-job-design)

**Test layer**: Setup / Verify (emulated or dev account)

### Setup

- [ ] EventBridge rule → Lambda `oci-registry-gc-aws` (or shared gc bin) with read/write to S3 + DynamoDB scan permissions

### Verify

- [ ] Manual invoke dry-run in dev account logs candidate count

---

## 4.6 — GC scheduled deployment (Azure)

**Architecture**: [architecture-operations.md](../architecture-operations.md)

**Test layer**: Setup / Verify

### Setup

- [ ] Timer-triggered Function with managed identity to Blob + Cosmos

### Verify

- [ ] Dry-run in dev subscription

---

## 4.7 — Observability completion

**Architecture**: [architecture-observability.md](../architecture-observability.md)

**Test layer**: HTTP + mock → smoke

### Test first

- [ ] Counters/histograms for presign failures, GC deleted/failed, handler error codes by OCI code
- [ ] No manifest bodies, JWTs, or presigned URLs in any log/metric dimension

### Implement

- [ ] Wire CloudWatch / Azure Monitor exporters per cloud bin
- [ ] Dashboard sketch in `dev/README.md` (optional)

---

## 4.8 — Operator runbook

**Architecture**: [architecture-operations.md](../architecture-operations.md)

**Test layer**: Verify (documentation review)

### Implement

- [ ] Runbook sections: enable content management in CI, GC dry-run → live, race window, revoke tokens vs delete manifests, rollback deploy
- [ ] Link from [plan.md](./plan.md) and crate README

### Verify

- [ ] Another engineer can execute dry-run GC on local data dir using only README + runbook

---

## 4.9 — Release confidence (optional)

**Architecture**: [guidelines-conformance.md#client-tool-matrix](../guidelines-conformance.md#client-tool-matrix)

**Test layer**: live / manual

### Test first

- [ ] `crane copy` smoke against deployed registry (nightly)
- [ ] Pre-release: containerd/buildkit pull only if product claims k8s pipeline support

### Notes

`docker login` / token service remain out of scope ([architecture-auth.md](../architecture-auth.md)).
