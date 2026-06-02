# oci-registry — implementation guidelines

Normative **principles, patterns, and engineering rules** for building the OCI registry. Applies across clouds and delivery slices.

| Need | Read |
|------|------|
| **What** the system is (traits, flows, boundaries, HTTP contract) | [`architecture.md`](./architecture.md) and linked `architecture-*.md` |
| **How to test** | [`guidelines-conformance.md`](./guidelines-conformance.md) |

Delivery checklists (phases, endpoints, task lists) are maintained separately; they reference this file and must not duplicate it.

---

## Plan task shape

Every **work unit** in a phase delivery checklist uses the same structure so tasks stay scannable and TDD is explicit.

### Phase document skeleton

```markdown
# Phase N — <short title>

**Prerequisite**: prior delivery slice complete (document at phase header) · **Architecture**: <links to architecture-*.md>

## Goal
<one paragraph — outcome of this phase, not a task list>

## Prerequisites
- [ ] <gate before starting this phase — workspace, prior phase, tooling>

## Exit criteria
- [ ] <observable “phase done” checks — conformance, deploy smoke, deferred behavior noted>

---

## N.M — <work unit title> [(end-<id>[, end-<id>])]

**Spec**: <link to distribution spec section, if endpoint-specific>
**Architecture**: <link to flows / backends / auth section>
**Test layer**: <mock | HTTP+mock | local | emulated | conformance — per guidelines-conformance>

### Test first
- [ ] <observable assertion — status, header, trait call, error code>

### Implement
- [ ] <minimal code or config to make tests pass>

### README (when runnable or configurable)
- [ ] <crate README and/or dev/README — only if this unit changes how to run something>

### Notes (optional)
<edge cases, emulator gaps, waivers — not checkbox work>
```

**Prerequisites** and **exit criteria** are phase-level gates (see [Delivery scope](#delivery-scope)). **Work units** (`N.M`) are the repeatable task shape.

### Example work unit

Illustrative example — blob pull via **307**:

```markdown
## 1.5 — Pull blob via 307 (end-2)

**Spec**: [GET Blob](https://github.com/opencontainers/distribution-spec/blob/v1.1.1/spec.md#get-blob)
**Architecture**: [`architecture-flows.md#pull-blob-layer`](./architecture-flows.md#pull-blob-layer)
**Test layer**: HTTP + mock traits → then local profile A

### Test first
- [ ] `GET /v2/{name}/blobs/{digest}` with `r` claim → **307**, `Location` is object-store host, `Docker-Content-Digest` matches digest
- [ ] `HEAD` same path → **307** (or **200** with headers only — match spec)
- [ ] Missing blob → **404** `BLOB_UNKNOWN`
- [ ] Mock **BS** `presign_get` called once; handler does not stream blob bytes

### Implement
- [ ] Handler: authZ → optional **BS** `head` → **BS** `presign_get` → **307** response
- [ ] Wire `S3BlobStore::presign_get` (and local FS equivalent for profile A)

### README (when runnable or configurable)
- [ ] Document that clients must not send `Authorization` to presigned `Location` URLs
```

### Checkbox rules

| Do | Don’t |
|----|--------|
| One observable outcome per `- [ ]` line | Vague items (“implement blob pull”) |
| Name **status codes**, **headers**, or **trait methods** when relevant | Duplicate architecture prose in the checklist |
| Link **architecture** (and spec for HTTP) in the work unit header | Embed delivery schedule inside a single task |
| Split **Test first** / **Implement** for behavior changes | Mix tests and implementation in one bullet |
| Add **README** bullets only when run/config surface changes | README-only phases with no test bullets |

Infrastructure-only units (compose, SAM, conformance scripts) may use **Setup** / **Verify** instead of Test first / Implement, but keep the same header metadata (`**Test layer**`, architecture links).

---

## Engineering principles

### Test-driven development (required)

For every behavior change:

1. **Write or extend a test first** (unit, trait storage, HTTP, or integration as appropriate).
2. Run it — expect **fail**.
3. Implement the **minimal** code to pass.
4. Run again — expect **pass**.
5. Refactor only when tests stay green.

Use the **lowest test layer** that can prove the behavior ([`guidelines-conformance.md`](./guidelines-conformance.md)). Do not skip lower layers because conformance passed.

### Delivery scope

- Respect the **prerequisites** and **exit criteria** documented for the current delivery slice before starting the next.
- Do not ship HTTP surface or behavior reserved for a **later** slice unless the active checklist explicitly includes it.
- When behavior or run instructions change, update [`catalog/oci-registry/README.md`](../../oci-registry/README.md) in the **same change** (binaries, features, env vars, ports, `dev/` entry points). Long emulator detail may live in `dev/README.md`; the crate README must still show **how to start** each workflow.

### Design alignment

- **Handlers depend on traits only** — no cloud SDK or storage key types in HTTP logic ([`architecture-overview.md`](./architecture-overview.md)).
- **Two traits, never one** — `BlobStore` (global CAS bytes) and `RegistryMetadata` (per-repo index). Orchestration per operation: [`architecture-flows.md#endpoint--trait-matrix`](./architecture-flows.md#endpoint--trait-matrix).
- **One deployable binary per cloud target** — separate `[[bin]]` + `required-features`; never `aws` + `azure` in one artifact.
- **Self-contained crate** — dev stacks and conformance under `catalog/oci-registry/dev/`; no dependency on repo `integration-tests` or `registry-core`.

---

## Patterns

### Blob upload modes

Two upload patterns; both may exist in the codebase. Ship each pattern only when the current delivery slice calls for it.

| Pattern | Traits | Rule |
|---------|--------|------|
| **Registry-mediated** | **RM** upload session + **BS** `put` | Chunks through compute SDK; registry validates digest on complete |
| **Presigned offload** | **RM** session + **BS** `presign_put` | Large bytes go client → object store; terminal `PUT ?digest=` on registry host for digest check and **201** |

### Testability before cloud

Provide in-memory or local (**FS + SQLite**) implementations of **both** traits so HTTP tests run without emulators or cloud accounts.

### Auth (implementation rule)

Do not collapse **AuthN** (edge JWT) and **AuthZ** (repo claims in handler). Claim format, mount rules, and MVP client model: [`architecture-auth.md`](./architecture-auth.md).

For local and CI: mint test JWTs with production claim strings, or a **documented** dev bypass — never unauthenticated production behavior.

### Observability (implementation rule)

On successful tag manifest GET, emit structured `oci_tag_download` per [`architecture-observability.md`](./architecture-observability.md). Never log manifest bodies, JWTs, or presigned URLs.

---

## Rules (quick reference)

| Rule | Detail in architecture |
|------|-------------------------|
| Spec wins over OpenAPI | [`architecture-http.md`](./architecture-http.md) |
| System in/out of scope | [`architecture-overview.md#system-boundaries`](./architecture-overview.md#system-boundaries) |
| Crate layout, dedicated `/v2/` host | [`architecture-overview.md`](./architecture-overview.md), [`architecture-edge.md`](./architecture-edge.md) |
| Trait shapes, presign TTL, GC-only `delete` | [`architecture-traits.md`](./architecture-traits.md) |
| Manifest gzip, item budget, backend keys | [`architecture-backends.md`](./architecture-backends.md) |
| DELETE flows, global blob refcheck | [`architecture-flows.md`](./architecture-flows.md) |
| GC job, dry-run, runbook | [`architecture-operations.md`](./architecture-operations.md) |
| Tag download + error/presign metrics | [`architecture-observability.md`](./architecture-observability.md) |
| **307** blob pull, upload `Location` hosts | [`architecture-http.md`](./architecture-http.md), [`architecture-flows.md`](./architecture-flows.md) |

---

## Anti-patterns

| Don’t | Do instead |
|-------|------------|
| One binary with `aws` + `azure` features | Separate `[[bin]]` per cloud |
| Stream large layers through Lambda | **307** + presigned GET ([`architecture-http.md`](./architecture-http.md)) |
| Put DynamoDB/Cosmos key names in trait signatures | Hide keys inside `src/storage/*` |
| Trust OpenAPI alone for upload/**307** behavior | Distribution spec + conformance |
| Anonymous pull “for convenience” | JWT on every `/v2/*` route |
| Route OCI under `/catalog/v1/` or another prefix | Dedicated registry host; `/v2/` at root |
| Couple to `integration-tests` crate | Keep stacks in `catalog/oci-registry/dev/` |
| Skip unit/HTTP tests because conformance passed | Full pyramid in [`guidelines-conformance.md`](./guidelines-conformance.md) |

---

## Related docs

| Doc | Purpose |
|-----|---------|
| [`guidelines-conformance.md`](./guidelines-conformance.md) | Test layers, conformance profiles, CI pyramid |
| [`architecture.md`](./architecture.md) | Architecture index |
