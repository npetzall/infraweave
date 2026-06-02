# oci-registry — conformance and testing guidelines

**Principles and rules** for verifying the OCI Distribution registry at each layer. Runnable commands, ports, and env vars live in [`catalog/oci-registry/README.md`](../../oci-registry/README.md). When a profile or gate is required for a delivery slice is defined in that slice’s checklist—not here.

**Spec gate**: [distribution-spec/conformance](https://github.com/opencontainers/distribution-spec/tree/v1.1.1/conformance) (Go), pinned at **`@v1.1.1`**. Conformance exercises **HTTP** (`/v2/…`); it does not replace unit or in-process HTTP tests.

---

## Principles

1. Run **as much as possible locally** — mock traits or real trait impls behind the production HTTP stack.
2. **Deployed cloud is smoke**, not the primary gate. No AWS/Azure secrets on PR CI.
3. Use the **lowest layer** that can prove the behavior; ascend only when the layer below cannot cover it.
4. All runner scripts, compose files, and bootstrap live under **`catalog/oci-registry/dev/`** — **no** dependency on the repo `integration-tests` crate.
5. Auth for local runs: mint test JWTs with `infraweave_oci::<repo>::r|rw` ([`architecture-auth.md`](./architecture-auth.md)), or a documented dev bypass — not unauthenticated production behavior.
6. Follow [`guidelines.md`](./guidelines.md) TDD: failing test → minimal pass → refactor.

---

## Test layers (bottom to top)

```text
1. Mock traits        → unit + upload state machine + GC logic
2. HTTP + mock traits → status codes, headers, authZ routing
3. Local backends     → FS + SQLite via oci-registry-local
4. Emulated cloud     → MinIO, DynamoDB Local, Azurite, Cosmos emulator + cloud SDK binaries
5. Live cloud         → post-merge / nightly / release smoke only
6. Official conformance suite → black-box HTTP against whichever server is listening (layers 3–5)
```

### Layer reference

| Layer | Setup | Valid for | Invalid for |
|-------|--------|-----------|-------------|
| **Mock `BlobStore` + `RegistryMetadata`** | In-memory or test doubles in Rust | Unit tests, handler orchestration with injected stores | Replacing distribution HTTP API coverage |
| **HTTP + mock traits** | In-process Axum `Router` + mock stores | Table-driven status codes, error JSON, authZ matrix | S3 presign signatures, DynamoDB transact |
| **Local backends** | `oci-registry-local` + FS + SQLite (`local` feature) | Fast PR gate; handler + metadata/blob orchestration | Proving cloud SDK presign / transact paths |
| **Emulated cloud** | Conformance profiles B/C (see below) | SDK code paths, **307** + presigned GET/PUT, metadata transact | Production Cognito / APIM at edge |
| **Live cloud** | Deployed AWS/Azure/GCP | Edge JWT authorizer, real S3 CORS/TTL, API GW limits | Every PR — too slow, needs secrets |
| **Official conformance suite** | Go suite → `OCI_ROOT_URL` | End-to-end spec compliance over HTTP | Unit-testing storage traits in isolation |
| **`integration-tests` crate** | Platform-wide env tests | Other catalog components | **Not** used by oci-registry |

---

## Official conformance suite

**Source**: [distribution-spec/conformance](https://github.com/opencontainers/distribution-spec/tree/v1.1.1/conformance)

Run via GitHub Action, `go test`, or `dev/run-conformance-*.sh`. Pin the conformance checkout at **v1.1.1** to match the distribution spec.

### Shared env (all profiles)

```text
OCI_NAMESPACE=conformance/test
OCI_TEST_PULL=1
OCI_TEST_PUSH=1
OCI_TEST_CONTENT_DISCOVERY=1
OCI_TEST_CONTENT_MANAGEMENT=0   # enable when DELETE is implemented and in scope for the slice
# Auth: Bearer JWT infraweave_oci::conformance/test::rw (local mint) or OCI_AUTH_BYPASS in dev
```

**Content management (`OCI_TEST_CONTENT_MANAGEMENT`)**: run with **`0`** until the registry implements spec-compliant **DELETE** for manifests and blobs. While `0`, the suite skips DELETE tests; the registry may return **405** on DELETE. When DELETE is in scope for the delivery slice, set **`1`** and require the DELETE suite green.

### Conformance profiles

Same Go suite; different binary + backends. Ports are stable conventions (document any change in the crate README).

| Profile | Server | Feature | Backends | Port | Proves |
|---------|--------|---------|----------|------|--------|
| **A — local** | `oci-registry-local` | `local` | FS + SQLite | `5000` | Handlers + trait orchestration on local storage |
| **B — aws** | `sam local start-api` → `oci-registry-aws` | `aws` | MinIO + DynamoDB Local | `5001` | AWS SDK paths, presigned **307**, Lambda packaging |
| **C — azure** | `oci-registry-azure` | `azure` | Azurite + Cosmos emulator | `5002` | Azure SDK + SAS |

| Variable | Profile A | Profile B | Profile C |
|----------|-----------|-----------|-----------|
| `OCI_ROOT_URL` | `http://127.0.0.1:5000` | `http://127.0.0.1:5001` | `http://127.0.0.1:5002` |
| Script | `dev/run-conformance-local.sh` | `dev/run-conformance-aws.sh` | `dev/run-conformance-azure.sh` |

Example starts (detail in crate README):

```text
# A: cargo run --bin oci-registry-local --features local  →  :5000
# B: docker compose -f dev/docker-compose.aws.yml up && dev/bootstrap-aws.sh && sam local start-api -p 5001
# C: docker compose -f dev/docker-compose.azure.yml up && dev/bootstrap-azure.sh && cargo run --bin oci-registry-azure --features azure  →  :5002
```

**CI rule**: Profile **A** is the default PR gate once HTTP exists. Add **B** before merge when AWS local stack is stable. Add **C** when the Azure binary and emulators are in scope for the active delivery slice.

---

## CI pyramid

```text
PR (required, no cloud account)
  → Rust unit tests (mock traits)
  → HTTP table tests (in-process Router + mock stores)
  → conformance profile A (:5000)
  → conformance profile B (:5001)     [when AWS local gate is active]
  → conformance profile C (:5002)   [when Azure is in scope]

Post-merge / nightly (optional)
  → conformance vs deployed AWS/Azure (real Cognito + S3/Dynamo)
  → crane smoke one image

Release / manual
  → containerd or buildkit if k8s compatibility claimed
```

### CI workflow jobs

Use **split jobs**, not one monolithic workflow:

| Job | Contents |
|-----|----------|
| **`conformance-local`** | `cargo test -p oci-registry` → profile A → Go conformance |
| **`conformance-aws`** | compose → bootstrap → `sam build` → profile B |
| **`conformance-azure`** | compose → bootstrap → profile C |

Do not invoke the `integration-tests` crate for oci-registry gates.

---

## Behaviors that need emulated or live backends

| Behavior | Minimum layer | Optional live smoke |
|----------|---------------|---------------------|
| Real S3 presigned GET signature, TTL, CORS | Profile B (MinIO) | Nightly on real S3 |
| DynamoDB transact / Cosmos batch | Trait tests vs Local / emulator | Nightly |
| API GW upload size limit | Deployed smoke or documented cap test | — |
| JWT + claim authZ | Unit tests for claim parser | Conformance with test JWTs |
| Production Cognito / APIM at edge | Live cloud only | Nightly |

---

## Client tool matrix

| Tool | Role | When |
|------|------|------|
| **distribution-spec conformance** | Spec gate | PR: profiles per CI pyramid above |
| **`crane`** | Human/smoke confidence | Nightly vs deployed env (optional PR) |
| **`go-containerregistry` / `reggie`** | Integration helpers | In-process or local profile smoke |
| **containerd / buildkit** | Kube pipelines | Pre-release only if needed |

---

## Integration tests (non-conformance)

Between unit tests and the Go suite:

- **Trait / storage**: `BlobStore` / `RegistryMetadata` against FS, SQLite, MinIO, DynamoDB Local, Azurite, Cosmos emulator.
- **SAM local**: `oci-registry-aws` with compose + `sam local` when profile B is in use.
- **HTTP**: Axum router + mock traits; table-driven status codes and headers.
- **Redirect**: assert clients do not send `Authorization` to presigned object-store URLs.

Record any conformance tests skipped while `OCI_TEST_CONTENT_MANAGEMENT=0`.

---

## Related docs

| Doc | Purpose |
|-----|---------|
| [`guidelines.md`](./guidelines.md) | Implementation principles (TDD, scope, patterns) |
| [`architecture-auth.md`](./architecture-auth.md) | JWT claims for conformance auth |
| [`catalog/oci-registry/README.md`](../../oci-registry/README.md) | Runnable commands, ports, env vars |
