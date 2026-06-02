# Architecture — operations (GC, runbook)

Part of [oci-registry architecture](./architecture.md).

Background **garbage collection**, operator runbook expectations, and the relationship between spec **DELETE** APIs and blob lifecycle. HTTP DELETE flows: [architecture-flows.md](./architecture-flows.md#delete-manifest-end-9). Trait rules: [architecture-traits.md](./architecture-traits.md). Global blob pool: [architecture-backends.md](./architecture-backends.md).

---

## Blob lifecycle

| Stage | Where bytes live | Metadata |
|-------|------------------|----------|
| Push complete | **BlobStore** (global CAS) | `link_blob_to_repo` + manifest `References` on `DIGEST#` rows |
| Manifest delete | **BlobStore** unchanged | Tag/digest rows removed; `References` updated |
| Blob DELETE API | **BlobStore** if globally unreferenced | Repo link removed |
| GC job | **BlobStore** delete for orphans | Consistency scan only — GC does not delete manifest rows |

Manifest delete **never** synchronously deletes layer bytes. Orphan blobs (no manifest references anywhere) are reclaimed by the **GC job** or an explicit **DELETE blob** when refcheck passes.

---

## GC job design

Run GC as a **scheduled worker** separate from request-path Lambda/Functions — not inline on every DELETE.

| Deployment | Typical trigger |
|------------|-----------------|
| AWS | EventBridge → Lambda on cron (e.g. daily off-peak) |
| Azure | Timer trigger on Functions or external worker with managed identity |
| Local / CI | CLI subcommand or one-shot script for dev |

### Algorithm (conceptual)

1. **Enumerate candidate digests** — scan metadata for blobs in `BlobStore` with no incoming `References` from any `DIGEST#` row in any repo, **or** process a **tombstone queue** populated when manifests/blobs are deleted.
2. **Re-verify** each candidate immediately before delete (refcheck race window — see below).
3. **`BlobStore::delete(digest)`** in batches with configurable rate limits (S3 DeleteObject throughput, cost).
4. **Log** digest, key, outcome; increment success/failure metrics ([`architecture-observability.md`](./architecture-observability.md)).

Prefer **scan + refcheck** for MVP simplicity. A **tombstone queue** (SQS / Service Bus row per deleted digest) reduces full-table scans at scale — optional optimization.

### Dry-run mode

Support **`OCI_GC_DRY_RUN=1`** (or CLI `--dry-run`):

- List object keys / digests that **would** be deleted.
- Do **not** call `BlobStore::delete`.
- Emit structured log `event=oci_gc_dry_run` with count and sample digests (no blob bodies).

Dry-run is required for operator confidence before first production GC and after schema migrations.

### Rate limits and batching

| Control | Purpose |
|---------|---------|
| Max deletes per run | Cap blast radius |
| Pause between batches | Avoid S3/Blob throttling |
| Max run duration | Lambda timeout — checkpoint and resume on next schedule if needed |

Document defaults in the crate README when the GC entrypoint ships.

---

## Races and consistency

| Race | Safe behavior |
|------|---------------|
| New manifest references digest **during** GC | Re-verify refcheck immediately before each `delete`; skip if references appeared |
| Manifest delete + GC same digest | DELETE API removes metadata first; GC only sees orphan after refcheck |
| Two repos, shared digest, delete manifest in A only | Blob **retains** — B still references |
| GC deletes blob while client pulls | Client follows **307**; object store **404** → client re-GETs registry for fresh presign ([`architecture-flows.md`](./architecture-flows.md#pull-blob-layer)) |

GC is **eventually consistent**. A brief window may exist where metadata shows no references but a in-flight push has not yet committed — re-verify refcheck mitigates this. Do not delete on tag-row removal alone; always use manifest `References` graph on `DIGEST#` rows ([`architecture-backends.md`](./architecture-backends.md)).

---

## Operator runbook (expectations)

Document in the crate README **Operations** section when GC and DELETE ship:

| Topic | Operator action |
|-------|-----------------|
| **Schedule** | Cron expression / timer config; default off-peak |
| **Dry-run** | Run dry-run after deploy; review log count before live delete |
| **Abort** | Disable schedule / cancel in-flight run; partial batch deletes are safe (idempotent `delete`) |
| **Restore** | If S3 **versioning** enabled on blob bucket, restore deleted object by version id; metadata must be re-pushed separately — GC does not restore tags |
| **Conformance** | Set `OCI_TEST_CONTENT_MANAGEMENT=1` when DELETE is live ([`guidelines-conformance.md`](./guidelines-conformance.md)) |
| **Staging chaos** | Run GC while push/pull integration test to validate race handling before production |

---

## Related

| Doc | Topics |
|-----|--------|
| [architecture-flows.md](./architecture-flows.md#delete-manifest-end-9) | DELETE manifest/blob HTTP semantics |
| [architecture-observability.md](./architecture-observability.md) | GC and error metrics |
| [architecture-traits.md](./architecture-traits.md) | `BlobStore::delete` GC-only |
