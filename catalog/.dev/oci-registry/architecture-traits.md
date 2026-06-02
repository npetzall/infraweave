# Architecture — traits

Part of [oci-registry architecture](./architecture.md).

Traits split **bytes** from **metadata** so object stores and KV/document DBs are interchangeable without changing HTTP logic.

## `BlobStore` — content-addressable global pool

```rust
/// Content-addressable blob pool (global, not per-repo).
#[async_trait]
trait BlobStore: Send + Sync {
    async fn head(&self, digest: &Digest) -> Result<BlobMeta>;
    async fn get_stream(&self, digest: &Digest) -> Result<ByteStream>;
    async fn put(&self, digest: &Digest, body: Bytes) -> Result<()>;
    async fn delete(&self, digest: &Digest) -> Result<()>; // GC only

    /// OCI pull offload — 307 to presigned GET (S3) or SAS (Azure).
    async fn presign_get(&self, digest: &Digest, ttl: Duration) -> Result<Url>;

    /// Presigned PUT for large upload offload (optional capability).
    async fn presign_put(&self, digest: &Digest, ttl: Duration, size: u64) -> Result<Url>;
}
```

**Object key layout**:

```text
v2/blobs/sha256/<first-two-hex>/<full-digest>/data
```

| Method | Typical caller | Notes |
|--------|----------------|-------|
| `head` | Upload dedupe, existence checks | Before client uploads bytes |
| `put` | Registry-hosted PATCH/PUT via compute SDK | Digest verified on write |
| `presign_get` | `GET` blob → **307** | Large layers never stream through Lambda; TTL typically **15–60+ minutes** |
| `presign_put` | Large upload offload | Time-limited PUT URL; terminal digest validation stays on registry host |
| `delete` | GC job | Only when metadata refcount is zero — see [architecture-operations.md](./architecture-operations.md) |

## `RegistryMetadata` — indexed namespace

```rust
/// Repository namespace + upload sessions + GC graph + referrers.
#[async_trait]
trait RegistryMetadata: Send + Sync {
    async fn get_manifest(&self, repo: &str, reference: &str) -> Result<ManifestRecord>;
    async fn put_manifest(&self, repo: &str, reference: &str, digest: &Digest, media_type: &str) -> Result<()>;
    async fn delete_manifest(&self, repo: &str, reference: &str) -> Result<()>;
    async fn list_tags(&self, repo: &str, n: u32, last: Option<&str>) -> Result<TagPage>;
    async fn link_blob_to_repo(&self, repo: &str, digest: &Digest) -> Result<()>;
    async fn list_referrers(&self, repo: &str, subject: &Digest, artifact_type: Option<&str>) -> Result<ImageIndex>;

    async fn create_upload(&self, repo: &str) -> Result<UploadSession>;
    async fn get_upload(&self, session_id: &Uuid) -> Result<UploadSession>;
    async fn update_upload_range(&self, session_id: &Uuid, offset: u64) -> Result<()>;
    async fn complete_upload(&self, session_id: &Uuid, digest: &Digest) -> Result<()>;
}
```

### Design rules

- Trait methods are **cloud-agnostic** — no DynamoDB/Cosmos key names in the public API. GCP backend selection is an implementation detail behind the same trait (GCP metadata backend deferred).
- `reference` is a tag name or manifest digest string from the URL path.
- `list_tags` must return **lexical order** with stable `last` cursor.
- `put_manifest` implementations use **atomic multi-item writes** (DynamoDB `TransactWriteItems`, Cosmos **transactional batch** on same partition).

## Supporting types (conceptual)

| Type | Fields (illustrative) |
|------|------------------------|
| `Digest` | `algorithm`, `hex` (e.g. `sha256:abc…`) |
| `BlobMeta` | `size`, `exists` |
| `ManifestRecord` | `payload` (compressed bytes), `media_type`, `digest`, `references` (for GC) |
| `UploadSession` | `uuid`, `repo`, `offset`, `expected_digest`, `expires_at` |
| `TagPage` | `tags[]`, `next_last` |

**Implementations**: [AWS & Azure backends](./architecture-backends.md) · [per-request usage](./architecture-flows.md)
