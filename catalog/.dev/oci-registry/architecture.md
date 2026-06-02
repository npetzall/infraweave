# oci-registry — architecture

Index for the OCI Distribution registry architecture. **GCP metadata backend is deferred** (follow-on cloud) — see [architecture-backends.md](./architecture-backends.md#gcp-deferred).

## Architecture docs

| Doc | Topics |
|-----|--------|
| [architecture-overview.md](./architecture-overview.md) | Two-layer storage model, crate layout, system boundaries, `/v2/` dedicated host |
| [architecture-http.md](./architecture-http.md) | HTTP errors, headers, redirects, spec vs OpenAPI sources of truth |
| [architecture-edge.md](./architecture-edge.md) | DNS, dedicated host + separate API (`/v2/` at host root), API GW/APIM, Lambda/Functions |
| [architecture-traits.md](./architecture-traits.md) | `BlobStore` and `RegistryMetadata` trait shapes, design rules |
| [architecture-flows.md](./architecture-flows.md) | Per-request flows (blob pull **307**, upload modes, DELETE), sequence diagrams, endpoint → trait matrix |
| [architecture-backends.md](./architecture-backends.md) | AWS S3/DynamoDB and Azure Blob/Cosmos shapes, local & GCP notes |
| [architecture-cost.md](./architecture-cost.md) | Cost, performance, and selection rationale |
| [architecture-operations.md](./architecture-operations.md) | GC job, dry-run, races, operator runbook |
| [architecture-observability.md](./architecture-observability.md) | Tag download, error/presign/GC metrics, CloudWatch & Azure monitoring |
| [architecture-auth.md](./architecture-auth.md) | Edge JWT, Cognito, client credential flows |

## Related docs

| File | Content |
|------|---------|
| [`guidelines.md`](./guidelines.md) | Implementation principles and engineering rules |
| [`guidelines-conformance.md`](./guidelines-conformance.md) | Conformance and testing layers |
