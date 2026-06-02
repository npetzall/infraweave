# Phase 4 — GCP (optional)

**Parent**: [`plan.md`](./plan.md) · **Prerequisite**: [`plan-3.md`](./plan-3.md) complete (or **plan-2** if DELETE/GC deferred — document waiver) · **Architecture**: [`architecture-backends.md`](../architecture-backends.md#gcp-deferred)

## Goal

Add `gcp` feature to the **same** `oci-registry` crate and `oci-registry-gcp` `[[bin]]` with **GCS** `BlobStore` and a **RegistryMetadata** backend chosen in §4.0, without changing public trait methods.

## Exit criteria

- [ ] Same HTTP/API behavior as AWS MVP (Phase 1) + agreed Phase 2/3 subset on GCP staging
- [ ] Conformance run against GCP-backed local emulator or staging with test JWT
- [ ] GCP metadata backend documented (Firestore, Spanner, Cloud SQL, etc.) in [`architecture-backends.md`](../architecture-backends.md#gcp-deferred)
- [ ] Trait layer still free of GCP-specific key names in public API

---

## 4.0 — Resolve metadata backend

### Test first

- [ ] Spike doc or ADR with latency/cost comparison for: tag pull, manifest put transact, referrers query, upload session

### Implement

- [ ] Update [`architecture-backends.md`](../architecture-backends.md#gcp-deferred) (and `gcp-manifest.md` when schema is fixed)
- [ ] Schema doc: `gcp-manifest.md` (create when decided)

---

## 4.1 — GCS `BlobStore`

### Test first

- [ ] Fake GCS or emulator: `put`/`head` at `v2/blobs/sha256/…/data`
- [ ] Signed URL GET → **307** pull flow matches S3 integration test
- [ ] Signed URL PUT upload (Phase 2) if GCP ships with full parity

### Implement

- [ ] `GcsBlobStore` using `google-cloud-storage` or official SDK
- [ ] `presign_get` / `presign_put` via V4 signed URLs

---

## 4.2 — GCP `RegistryMetadata`

### Test first

- [ ] Round-trip manifest by tag and digest (same tests as Phase 0 §0.6, ported)
- [ ] `TransactWrite` equivalent: atomic tag + digest write
- [ ] Referrers query matches Phase 2 behavior

### Implement

- [ ] Backend per §4.0 decision
- [ ] Read-optimized gzip duplication strategy aligned with [`architecture-cost.md`](../architecture-cost.md)

---

## 4.3 — GCP edge

### Test first

- [ ] Request without identity token → **401**
- [ ] Valid token + claim → handler invoked

### Implement

- [ ] Cloud Run or Cloud Functions + HTTPS load balancer + IAP or Firebase JWT (equivalent to Cognito JWT at edge)
- [ ] Dedicated hostname for OCI — see [`architecture-edge.md`](../architecture-edge.md)

---

## 4.4 — Binary and CI

### Test first

- [ ] `cargo test -p oci-registry --features gcp`
- [ ] Staging smoke: push/pull with pre-provisioned credential per [`architecture-auth.md`](../architecture-auth.md)

### Implement

- [ ] `oci-registry-gcp` bin + `gcp` feature module in `oci-registry`
- [ ] Optional nightly GCP smoke job
- [ ] **`catalog/oci-registry/README.md`**: **GCP** / `gcp` feature — build commands, `oci-registry-gcp`, local dev stack when defined, env vars

---

## README — Phase 4

- [ ] README: `oci-registry-gcp` binary, `gcp` feature, local conformance profile (if added), emulator endpoints per §4.0 decision
- [ ] Link to `gcp-manifest.md` or ADR when metadata backend chosen

---

## Phase 4 — Done checklist

- [ ] GCP metadata backend documented in [`architecture-backends.md`](../architecture-backends.md#gcp-deferred)
- [ ] All sections above checked or explicitly deferred with owner/date
- [ ] README checklist (above) complete
- [ ] [`plan.md`](./plan.md) index updated if scope changes
