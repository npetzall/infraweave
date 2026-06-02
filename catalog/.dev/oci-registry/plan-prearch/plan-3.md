# Phase 3 — Operations (GC, DELETE, metrics)

**Parent**: [`plan.md`](./plan.md) · **Prerequisite**: [`plan-2.md`](./plan-2.md) complete · **Architecture**: [`architecture-flows.md`](../architecture-flows.md#delete-manifest-end-9), [`architecture-operations.md`](../architecture-operations.md), [`architecture-observability.md`](../architecture-observability.md) · **GC graph**: [`architecture-traits.md`](../architecture-traits.md), [`architecture-backends.md`](../architecture-backends.md) (global blob pool)

## Goal

Make the registry operable at scale: safe **garbage collection**, spec **DELETE** APIs, structured **operational metrics**, and optional admin **`_catalog`**.

## Exit criteria

- [ ] `DELETE` manifest and blob behave per spec; blob delete refused when referenced
- [ ] GC job removes only unreferenced S3/Blob objects; metadata remains consistent
- [ ] Metrics/alarms for 4xx/5xx rate and presign failures documented + tested in staging
- [ ] Conformance DELETE tests pass (if enabled in suite)

---

## 3.1 — DELETE manifest (end-9)

**Architecture**: [`architecture-flows.md#delete-manifest-end-9`](../architecture-flows.md#delete-manifest-end-9)

### Test first

- [ ] `DELETE …/manifests/{ref}` → **202** Accepted (or spec-accurate status); tag and digest rows updated/removed
- [ ] Delete by digest vs tag covered
- [ ] Missing manifest → **404** `MANIFEST_UNKNOWN`
- [ ] Without `::rw` claim → **403**

### Implement

- [ ] `RegistryMetadata::delete_manifest`
- [ ] Update referrer rows / `SubjectDigest` edges as needed

---

## 3.2 — DELETE blob (end-10)

**Architecture**: [`architecture-flows.md#delete-blob-end-10`](../architecture-flows.md#delete-blob-end-10)

### Test first

- [ ] `DELETE …/blobs/{digest}` when still referenced by any manifest → **403** (or spec-appropriate denial)
- [ ] Unreferenced digest → **202**; object removed from `BlobStore`
- [ ] Global pool: other repos still referencing → deny

### Implement

- [ ] Refcount or scan `References` in metadata before `BlobStore::delete`
- [ ] `link_blob_to_repo` graph used for GC (dedupe across repos)

---

## 3.3 — GC job design

**Architecture**: [`architecture-operations.md`](../architecture-operations.md)

### Test first

- [ ] Fixture: two repos share digest; delete all manifests referencing digest in repo A only → blob remains
- [ ] Delete all references globally → GC candidate
- [ ] Dry-run mode lists keys without deleting

### Implement

- [ ] Scheduled Lambda / Functions timer / external worker
- [ ] Scan metadata for unreferenced digests (or tombstone queue)
- [ ] Batch `BlobStore::delete` with rate limits
- [ ] Document race: new manifest during GC (runbook in [`architecture-operations.md`](../architecture-operations.md))

---

## 3.4 — Operational metrics

**Architecture**: [`architecture-observability.md#operational-health-metrics`](../architecture-observability.md#operational-health-metrics)

### Test first

- [ ] Force **404** route → structured log or metric increment `oci_registry_4xx`
- [ ] Presign failure path → log `event=oci_presign_failure` (no URL in log)
- [ ] Unit test: metric filter pattern matches JSON log shape (AWS EMF optional)

### Implement

- [ ] Counters/histograms via `tracing` + CloudWatch EMF / App Insights ([`architecture-observability.md`](../architecture-observability.md))
- [ ] Dashboards: error rate, presign failures, `TagDownload` from Phase 1
- [ ] Alarms on 5xx spike

---

## 3.5 — Optional `_catalog`

### Test first

- [ ] If enabled: `GET /v2/_catalog` returns repository list for admin JWT only
- [ ] Disabled by default → **404** or not routed

### Implement

- [ ] Feature flag `OCI_ENABLE_CATALOG=1`
- [ ] Paginated repo enumeration from metadata scan (cost warning in ops doc)

---

## 3.6 — Conformance and ops runbook

### Test first

- [ ] Conformance DELETE suite green
- [ ] Chaos test: run GC while push/pull integration test (staging)

### Implement

- [ ] Runbook: GC schedule, dry-run, abort, restore from versioning (if S3 versioning on) — per [`architecture-operations.md`](../architecture-operations.md)
- [ ] **`catalog/oci-registry/README.md`**: **Operations** section — GC job how to run, DELETE enabled, `OCI_TEST_CONTENT_MANAGEMENT=1` per [`guidelines-conformance.md`](../guidelines-conformance.md), metrics/alarms pointers ([`architecture-observability.md`](../architecture-observability.md))
- [ ] README: optional `_catalog` flag (`OCI_ENABLE_CATALOG`) if implemented

---

## README — Phase 3

- [ ] README documents DELETE APIs, GC job invocation (CLI/env/schedule), operational metrics, conformance with `OCI_TEST_CONTENT_MANAGEMENT=1`
- [ ] Any new ops script or feature flag from §3.x in README same PR

---

## Phase 3 — Done checklist

- [ ] All sections above checked
- [ ] README checklist (above) complete
- [ ] Production-ready ops for AWS (and Azure if deployed)
- [ ] [`plan-4.md`](./plan-4.md) optional next
