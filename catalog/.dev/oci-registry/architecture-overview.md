# Architecture — overview

Part of [oci-registry architecture](./architecture.md).

OCI **Distribution Spec v1.1.1** registry: content-addressable blob storage plus indexed repository metadata, deployed on **AWS (Lambda + API Gateway + S3 + DynamoDB)** or **Azure (Functions + APIM/HTTP + Blob Storage + Cosmos DB)**. **GCP metadata backend is deferred** (follow-on cloud).

Two storage layers ([`oci_distribution_storage.md`](../../../docs_internal/specs/oci_distribution_storage.md)):

| Layer | Responsibility | Trait |
|-------|----------------|-------|
| **BlobStore** | Global, deduplicated layer/config bytes keyed by digest | `BlobStore` |
| **RegistryMetadata** | Tags, manifests, referrers, upload sessions, GC graph | `RegistryMetadata` |

HTTP handlers depend on traits only. Cloud-specific **binaries** wire concrete implementations — **one workspace crate**, feature-gated backends, **separate `[[bin]]` per cloud** (not separate packages per cloud):

```text
catalog/oci-registry/              # workspace member; single Cargo package
  Cargo.toml                         # [lib] + [[bin]] × N; features: aws | azure | local | gcp
  src/lib.rs                         # shared HTTP, traits, orchestration
  src/bin/aws.rs                     # oci-registry-aws  (required-features = ["aws"])
  src/bin/azure.rs                   # oci-registry-azure
  src/bin/local.rs                   # oci-registry-local
  src/storage/aws.rs                 # cfg(feature = "aws") — S3, DynamoDB
  src/storage/azure.rs               # cfg(feature = "azure")
  src/storage/local.rs               # cfg(feature = "local")
  dev/                               # self-contained local stacks; not integration-tests/
    docker-compose.aws.yml
    docker-compose.azure.yml
    bootstrap-aws.sh / bootstrap-azure.sh
    template.yaml (SAM), host.json + func (Azure)
    run-conformance-*.sh
  README.md
```

Do **not** build or deploy one artifact with multiple cloud features enabled. Local emulators and dev stacks live under this crate’s `dev/` directory (per-cloud compose, bootstrap scripts, conformance runners).

`oci-registry` stays **fully separate** from catalog `registry-core` and OpenTofu protocol registries — different spec, clients, conformance, APIGW, and CAS layout. Optional small shared utilities (presign, HTTP glue) may be extracted later without sharing storage traits.

OCI is exposed at **`/v2/`** on a **dedicated registry hostname** — the distribution spec fixes API paths at the host root, so OCI cannot share the catalog’s `/catalog/v1/` context-path layout. That implies a **separate HTTP API** (and DNS host) from the catalog API; detail in [architecture-edge.md](./architecture-edge.md#dedicated-hostname-and-separate-api).

## System boundaries

| In scope | Out of scope (until a new design decision says otherwise) |
|----------|-------------------------------------------------------------|
| `/v2/` per [Distribution Spec v1.1.1](https://github.com/opencontainers/distribution-spec/blob/v1.1.1/spec.md) | `/catalog/v1/` OpenTofu protocol registries |
| Trait-based `BlobStore` + `RegistryMetadata` | One **fat** binary with `aws` + `azure` in the same build |
| One crate `oci-registry` + per-cloud `[[bin]]` | Anonymous pull; Docker distribution **token service** (`GET /token`) |
| Cognito JWT at edge + repo claims in compute | Multi-GB blob bodies streamed through Lambda/Functions |
| Presigned blob I/O (pull via **307**; push via presigned PUT when implemented) | OCI mounted under a catalog path prefix |

Delivery **schedule** (which endpoints ship when) is out of scope for this overview; see [endpoint → trait matrix](./architecture-flows.md#endpoint--trait-matrix) for capability mapping.

**See also**: [traits](./architecture-traits.md), [HTTP contract](./architecture-http.md), [edge topology](./architecture-edge.md), [request flows](./architecture-flows.md), [backends](./architecture-backends.md), [auth](./architecture-auth.md).
