# Architecture — backends

Part of [oci-registry architecture](./architecture.md).

Trait implementations and database/document shapes for AWS, Azure, local, and deferred GCP. Trait definitions: [architecture-traits.md](./architecture-traits.md). Cost rationale: [architecture-cost.md](./architecture-cost.md).

## AWS — `BlobStore` (S3)

```rust
// oci-registry — cfg(feature = "aws") — conceptual
pub struct S3BlobStore {
    client: aws_sdk_s3::Client,
    bucket: String,
}

impl BlobStore for S3BlobStore {
    fn key(digest: &Digest) -> String {
        format!(
            "v2/blobs/{}/{}/{}/data",
            digest.algorithm(),
            &digest.hex()[..2],
            digest.hex()
        )
    }
    // head → HeadObject, put → PutObject, presign_get → presigning::PresigningConfig
}
```

## AWS — `RegistryMetadata` (DynamoDB single table)

Table: **`oci-registry-table`** — read-optimized manifest duplication (pull >> push: one `GetItem` per tag or digest pull).

| PK | SK | SubjectDigest (GSI-PK) | ManifestPayload | References | MediaType | TargetDigest |
|----|-----|------------------------|-----------------|------------|-----------|--------------|
| `REPO#my-app` | `TAG#latest` | — | gzip bytes | — | `application/vnd.oci…` | `sha256:abc…` |
| `REPO#my-app` | `TAG#v1.0.0` | — | *same gzip* | — | … | `sha256:abc…` |
| `REPO#my-app` | `DIGEST#sha256:abc…` | — | *same gzip* | `["sha256:layer1", …]` | … | — |
| `REPO#my-app` | `DIGEST#sha256:sig…` | `sha256:abc…` | gzip | `["sha256:sigLayer"]` | … | — |

- Store **`References`** only on `DIGEST#` rows (GC scans digest records; tag rows need not duplicate the graph).
- Set **`SubjectDigest`** only on `DIGEST#` rows for signatures (Cosign) or attestations (Buildx); GSI partition key for referrers.

**Upload session row** (same table):

| PK | SK | Offset | ExpiresAt |
|----|-----|--------|-----------|
| `REPO#my-app` | `UPLOAD#{uuid}` | `0` | ISO8601 |

**GSI_Referrers** (cost detail: [architecture-cost.md](./architecture-cost.md#referrers-gsi-aws)):

- Index name: `GSI_Referrers`
- GSI PK: `SubjectDigest`
- GSI SK: `PK` (repo scope)
- Projection: **KEYS_ONLY**
- Query: `SubjectDigest = :digest AND PK = REPO#<repo>` for `GET /v2/{name}/referrers/{digest}`

```rust
// RegistryMetadata — DynamoDB impl sketch
async fn get_manifest(&self, repo: &str, reference: &str) -> Result<ManifestRecord> {
    let sk = if reference.starts_with("sha256:") {
        format!("DIGEST#{reference}")
    } else {
        format!("TAG#{reference}")
    };
    let item = self.table.get_item()
        .key("PK", format!("REPO#{repo}"))
        .key("SK", sk)
        .send().await?;
    // decompress ManifestPayload; return TargetDigest for header
}
```

**Write path (manifest)**:

1. Hash **raw** JSON → `TargetDigest` (immutable).
2. Compress that exact raw text **once** (same bytes for every row).
3. `TransactWriteItems`: all `TAG#` rows (with `TargetDigest`) + one `DIGEST#` row with `References`.

**`Docker-Content-Digest`**: Response header must be the SHA256 of **raw** manifest bytes, not the gzip stored in `ManifestPayload`. On tag pull, read `TargetDigest` from the row and set the header while streaming the payload. Do not re-serialize JSON per row or compress independently per tag.

### Manifest payload size limit

Manifest bodies live in metadata as **gzip** in `ManifestPayload` (layers/config stay in `BlobStore`). Each tag or digest pull is one point read; **total item/document size** sets read cost.

| Cloud | One read unit (typical) | Item budget (default) |
|-------|-------------------------|------------------------|
| DynamoDB | **1 RCU** = strongly consistent read up to **4 KB** | **4096** B total item |
| Cosmos DB | **~1 RU** = point read for doc **&lt; ~1 KB** | **1024** B total item |

**Enforce on `PUT …/manifests/{ref}`** (before `TransactWriteItems` / transactional batch):

1. Hash and gzip the **raw** manifest JSON once (same bytes on every row).
2. For each row written (`TAG#…`, `DIGEST#…`), ensure the stored item fits the budget:

```text
len(ManifestPayload_gzip) ≤ item_budget_bytes − reserved_overhead(that_row)
```

`reserved_overhead` = serialized size of all **other** attributes on that row (`PK`, `SK`, `MediaType`, `TargetDigest`, optional `SubjectDigest`, `References` JSON on `DIGEST#` rows, DynamoDB/Cosmos encoding). Prefer measuring the real item (or marshalled map) in tests; use conservative constants in production if needed.

| Config | Default | Notes |
|--------|---------|--------|
| `OCI_MAX_MANIFEST_ITEM_BYTES` | **1024** | Cross-cloud default (Cosmos ~1 RU). AWS-only may set **4096** (1 RCU ceiling). |

**Reject** when gzip would exceed the budget for **any** row in the transaction → **413** (or OCI `SIZE_INVALID` / equivalent). Limit is on **compressed** `ManifestPayload`, not uncompressed JSON. Large OCI **image indexes** must gzip under this cap or push fails — they are not blob pulls.

**README** ([`catalog/oci-registry/README.md`](../../oci-registry/README.md), when the crate exists): document `OCI_MAX_MANIFEST_ITEM_BYTES`, default, formula, and that enforcement is on gzip payload after subtracting non-payload fields.

## Azure — `BlobStore` (Blob Storage)

```rust
pub struct AzureBlobStore {
    client: azure_storage_blobs::BlobServiceClient,
    container: String,
}

// Same path convention: v2/blobs/sha256/aa/full.../data
// presign_get → User Delegation SAS or account SAS
```

Container layout mirrors S3; one container per environment or prefix per tenant (infra choice).

## Azure — `RegistryMetadata` (Cosmos DB)

Container: **`manifests`**, partition key: **`/repository`**.

| AWS | Azure |
|-----|-------|
| S3 | Blob Storage |
| DynamoDB RCU/WCU | Cosmos DB RU |
| `PK` / `SK` | `id` + `/repository` partition |

Cosmos charges RUs from **document size on disk**. Keep gzip `manifestPayload` under ~1 KB when possible so point reads stay at **~1 RU** (uncompressed OCI JSON can be 10–15 KB and cost multiple RUs). Storage is ~$0.25/GB/mo; duplicating a ~2 KB gzip across tag + digest docs is negligible vs read savings.

**Tag document**

```json
{
  "id": "TAG-latest",
  "repository": "my-app",
  "type": "tag",
  "targetDigest": "sha256:abc123xyz",
  "mediaType": "application/vnd.oci.image.manifest.v1+json",
  "manifestPayload": "<base64 gzip>"
}
```

**Digest document**

```json
{
  "id": "DIGEST-sha256:abc123xyz",
  "repository": "my-app",
  "type": "digest",
  "mediaType": "application/vnd.oci.image.manifest.v1+json",
  "manifestPayload": "<same base64 gzip>",
  "references": ["sha256:layer1", "sha256:layer2"]
}
```

**Referrer / attestation document**

```json
{
  "id": "DIGEST-sha256:sig999",
  "repository": "my-app",
  "type": "digest",
  "subjectDigest": "sha256:abc123xyz",
  "manifestPayload": "...",
  "references": ["sha256:sigLayer"]
}
```

**Upload session document**

```json
{
  "id": "UPLOAD-550e8400-e29b-41d4-a716-446655440000",
  "repository": "my-app",
  "type": "upload",
  "offset": 1048576,
  "expiresAt": "2026-06-01T12:00:00Z"
}
```

```rust
async fn put_manifest(...) -> Result<()> {
    let batch = self.container
        .create_transactional_batch(PartitionKey::from(repo))
        .create_item(tag_doc)?
        .create_item(digest_doc)?;
    batch.execute().await?;
}
```

**Reads**: `ReadItemAsync` by `id` + partition key (`TAG-…` or `DIGEST-…`) — same single-hop pattern as DynamoDB.

**Writes**: **Transactional batch** on one partition (`repository`) for atomic tag + digest push (rolls back on any failure).

Query referrers: `WHERE c.repository = @repo AND c.subjectDigest = @subject` (composite index as needed).

## Local (`oci-registry-local` bin, `local` feature)

| Trait | Implementation |
|-------|----------------|
| `BlobStore` | Filesystem or MinIO |
| `RegistryMetadata` | SQLite |

Same `oci-registry` crate at `catalog/oci-registry/` — one workspace member, one cloud feature per binary. Conformance profiles A/B/C use `dev/` scripts in that crate — not `integration-tests` ([`guidelines-conformance.md`](./guidelines-conformance.md)).

## GCP (deferred)

| Trait | Planned |
|-------|---------|
| `BlobStore` | GCS + signed URLs |
| `RegistryMetadata` | Firestore vs Cloud SQL vs Spanner — choose when implementing the GCP backend |
