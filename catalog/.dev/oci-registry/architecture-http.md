# Architecture — HTTP contract

Part of [oci-registry architecture](./architecture.md).

How the registry speaks HTTP on the wire: errors, redirects, headers, and which documents define behavior. Request flows and trait usage: [architecture-flows.md](./architecture-flows.md). Edge hostname and `Location` rules: [architecture-edge.md](./architecture-edge.md).

---

## Sources of truth

| Source | Role |
|--------|------|
| [OCI Distribution Spec v1.1.1](https://github.com/opencontainers/distribution-spec/blob/v1.1.1/spec.md) | **Behavior** — status codes, headers, upload protocol, conformance expectations |
| [`oci_distribution_openapi_v1_1_1.yaml`](../../../docs_internal/specs/oci_distribution_openapi_v1_1_1.yaml) | **Route index only** — may omit upload paths, conformance request bodies, and blob **307** redirect responses |
| [`oci_distribution_storage.md`](../../../docs_internal/specs/oci_distribution_storage.md) | CAS + metadata storage model behind traits |

When OpenAPI and the distribution spec disagree, **follow the spec** and the official [conformance suite](https://github.com/opencontainers/distribution-spec/tree/v1.1.1/conformance) (pinned at v1.1.1).

---

## Errors

All registry failures that map to OCI HTTP errors use a single **`RegistryError`** type that serializes to:

```json
{"errors":[{"code":"…","message":"…", …}]}
```

Map internal failures to [distribution spec error codes](https://github.com/opencontainers/distribution-spec/blob/v1.1.1/spec.md#errors) (`NAME_UNKNOWN`, `BLOB_UNKNOWN`, `UNAUTHORIZED`, etc.). Do not invent ad-hoc JSON shapes per handler.

---

## Response conventions

| Operation | Status / headers | Notes |
|-----------|------------------|-------|
| Blob pull | **307** + `Location` (object-store presigned URL) + `Docker-Content-Digest` | Bytes flow client → object store; no `Authorization` on presigned GET |
| Manifest pull | **200** + body + `Docker-Content-Digest` | Header is hash of **raw** manifest bytes (`TargetDigest`), not gzip stored in metadata |
| Upload session `Location` | Registry **hostname** | Derived from `Host` + `X-Forwarded-Proto`; optional `REGISTRY_PUBLIC_URL` override for local/SAM — [architecture-edge.md#registry-public-url](./architecture-edge.md#registry-public-url) |
| Presigned blob PUT/GET `Location` | Object-store **hostname** | Not the registry host |
| Manifest too large for metadata row | **413** / `SIZE_INVALID` (or spec equivalent) | After gzip; see [manifest payload limit](./architecture-backends.md#manifest-payload-size-limit) |

Presigned URL **TTL**: long enough for slow clients and large layers — typically **15–60+ minutes**; configure per backend where the SDK allows.

---

## Compute adapter

Lambda / Functions / local binary share one **lib** router:

- Thin `src/bin/*` entrypoints adapt the edge event format.
- Reuse SDK clients (`Arc` / `OnceLock`) across warm invocations.
- Parse repository `{name}` via `ANY /v2/{proxy+}` — full name including `/` — [architecture-edge.md#compute--lambda--functions](./architecture-edge.md#compute--lambda--functions).

---

## Related

| Doc | Topics |
|-----|--------|
| [architecture-flows.md](./architecture-flows.md) | Per-endpoint sequences, endpoint → trait matrix |
| [architecture-edge.md](./architecture-edge.md) | Dedicated host, catalog vs OCI APIs, public URL |
| [architecture-auth.md](./architecture-auth.md) | JWT at edge, repo authZ in handler |
