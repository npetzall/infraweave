# Architecture — cost and performance

Part of [oci-registry architecture](./architecture.md).

Rationale behind trait split, read-optimized metadata, presigned pulls, and platform choices. Storage shapes: [backends](./architecture-backends.md).

## Why split traits and clouds per binary

| Decision | Rationale |
|----------|-----------|
| **BlobStore vs RegistryMetadata** | CAS blobs are O(1) path lookups; tags/referrers/GC need indexes |
| **307 + presigned GET** | Spec-allowed; keeps multi-GB layers off Lambda/API GW |
| **One crate, one cloud per binary** | Single `oci-registry` package with feature gates; each `[[bin]]` links only one cloud’s SDKs — smaller Lambda zips, simpler IAM |
| **No `registry-core` merge** | Different spec, clients, conformance, and storage layout |

## Metadata: duplicate compressed manifest on read path

Registries are **read-heavy** (pull >> push). Duplicating a small gzip manifest on each `TAG#` and `DIGEST#` row buys **single-hop point reads**.

| Metric | AWS DynamoDB | Azure Cosmos DB |
|--------|--------------|-----------------|
| Read unit | **1 RCU** per `GetItem` | **~1 RU** per point read (&lt;1 KB doc) |
| Storage cost | ~$0.25/GB/mo | ~$0.25/GB/mo |
| Example | 3 tags + 1 digest × 2 KB gzip ≈ 8 KB storage | Same duplication strategy |
| Write penalty | 4 WCUs vs 2 on push (fractions of a cent at 10k pushes/mo) | Transactional batch, same partition |
| Read benefit | Every tag/digest pull = 1 RCU, &lt;3 ms | Sub-10 ms point read |

**Compression**: Store **gzip** (or brotli) in `ManifestPayload` / `manifestPayload`. **Enforce** a max gzip size so each metadata item stays within one read unit: `max_gzip = item_budget − reserved_overhead` ([manifest payload size limit](./architecture-backends.md#manifest-payload-size-limit)); default item budget **1024** B (Cosmos), optional **4096** B on AWS.

### Referrers GSI (AWS)

Use **`KEYS_ONLY`** projection on `GSI_Referrers` — do not project heavy binary fields into the index.

| Cost component | Estimate (on-demand, moderate traffic) |
|----------------|----------------------------------------|
| GSI storage | ~154 bytes/row (PK + SK + SubjectDigest); 100k sigs ≈ 15 MB → **~$0.004/mo** |
| GSI writes | 1 WCU per referrer row insert; normal image pushes without `SubjectDigest` do not touch the GSI |
| GSI reads | 1 RCU per referrers query (tiny KEYS_ONLY payload); 1M checks/mo → **~$0.125** |
| **Total** | **~$0.14/mo** at 100k signatures + 1M referrer API calls |

Avoids full table scans or a separate Redis cache for referrers lookup.

## Blob path: deduplication and Lambda limits

| Constraint | Mitigation |
|------------|------------|
| Lambda ~6 MB response | Never inline layer bytes; **307** only |
| API GW ~10 MB request | Cap monolithic upload; chunked PATCH |
| Lambda timeout | Registry-hosted SDK writes for small chunks; multipart/presigned parts for large uploads |
| Cross-repo same digest | One S3/Blob object; **RM** `link_blob_to_repo` only |

## AWS vs Azure selection (platform, not “winner”)

Both stacks implement the **same traits** with equivalent economics:

| Concern | AWS | Azure |
|---------|-----|-------|
| Edge | API Gateway HTTP API + Cognito authorizer | APIM + Entra/Cognito-equivalent JWT |
| Compute | Lambda | Functions |
| Blob | S3 presigned GET | Blob SAS |
| Metadata | DynamoDB on-demand + GSI | Cosmos serverless RU |

Choose per deployment target; trait shape is fixed so HTTP and tests are shared.

## Auth cost at edge

JWT validation at **API Gateway/APIM** rejects bad tokens before Lambda billing. Repo checks are cheap string matches on claims in Lambda. Detail: [architecture-auth.md](./architecture-auth.md).
