# Architecture — authentication

Part of [oci-registry architecture](./architecture.md).

Edge diagrams: [architecture-edge.md](./architecture-edge.md).

**Supported clients (MVP)**: OpenTofu CLI (`tofu init` / `oci://`), Infraweave CLI, `oci-module` push/pull. **Not** a design target: `docker login`, `crane` challenge flow, distribution token service.

---

## Core rule: credentials are always pre-provisioned

The registry does **not** issue tokens via `WWW-Authenticate` → `realm` → `GET /token?service=&scope=` (Docker Hub style) in MVP.

Every supported client must send **`Authorization: Bearer <token>`** on **every** `/v2/*` request, where `<token>` is a **Cognito access token** (or test JWT locally) that already contains `infraweave_oci::<repo>::r|rw` claims. **Anonymous pull is not allowed.**

```text
┌─────────────┐     Cognito login (Hosted UI, AWS CLI, Infraweave app)     ┌──────────────┐
│ Human / CI  │ ────────────────────────────────────────────────────────► │ access_token │
└─────────────┘                                                             └──────┬───────┘
                                                                                     │
         Store token (pick one)                                                      │
              ├─ .tofurc  oci_credentials { access_token = "…" }  (OpenTofu)         │
              ├─ OCI_MODULE_TOKEN / --token  (oci-module)                            │
              ├─ Infraweave CLI session → Bearer on registry calls                 │
              └─ ~/.docker/config.json via oras login (tools using Docker-style)   │
                                                                                     │
         Every /v2 request                                                           ▼
┌─────────────┐   Authorization: Bearer <token>   ┌──────────┐   JWT valid   ┌─────────────┐
│ Client      │ ────────────────────────────────► │ API GW   │ ─────────────► │ oci-registry│
│ tofu/oras/… │                                   │ Cognito  │   + claims    │ Lambda      │
└─────────────┘                                   │ JWT auth │               │ repo authZ  │
                                                  └──────────┘               └─────────────┘
```

**Cognito is not performed by `oras login` or by API Gateway “login”.** Cognito runs in your IdP flow; API Gateway only **validates** the Bearer JWT on each registry HTTP call.

---

## Edge topology (AWS)

```text
DNS: registry.example.com
  → ACM + API Gateway custom domain (ApiMapping)
  → HTTP API "oci-registry-api" (separate from catalog api.example.com)
  → JWT authorizer (Cognito) on all /v2/* routes
  → Lambda oci-registry (repo claim authZ)
```

| Goal | Mechanism |
|------|-----------|
| No path fights with `/catalog/*` | **Second HTTP API** — see [architecture-edge.md](./architecture-edge.md) |
| Subdomain | Custom domain + ApiMapping |
| `Location` URLs | Forwarded `Host` + `X-Forwarded-Proto` |
| AuthN | Cognito JWT authorizer on all `/v2/*` |
| Client tokens | Pre-provisioned Bearer |

### Cognito JWT authorizer (production)

1. **User pool** + app client; pre-token Lambda adds `infraweave_oci::<repo>::r|rw` claims.
2. **JWT authorizer** on oci-registry HTTP API: `issuer`, `audience`, identity source `$request.header.Authorization`.
3. Attach to **all** `/v2/*` routes (including `GET /v2/`).
4. Valid request → Lambda receives `requestContext.authorizer.jwt.claims`.
5. Invalid/missing token → API Gateway **401/403** (Lambda not run).

| Layer | Responsibility |
|-------|----------------|
| **Dedicated HTTP API** + `registry.example.com` | `/v2/*` only, separate from catalog API |
| **JWT authorizer** | Cognito user pool `issuer` + app client `audience`; identity source `$request.header.Authorization` |
| **All `/v2/*` routes** | `AuthorizationType: JWT` — missing/invalid token → **401/403 at edge** (Lambda not invoked) |
| **oci-registry Lambda** | Repo **authZ**: match `/v2/{name}/` to `infraweave_oci::…` claims in `requestContext.authorizer.jwt.claims` |

No anonymous pull. No registry-hosted token exchange in MVP.

### Claim format

Permissions are expressed as strings (one claim may hold multiple values):

```text
infraweave_oci::<repository>::r     # read  — pull, tags/list, referrers GET, HEAD
infraweave_oci::<repository>::rw    # read/write — push, PATCH upload, PUT manifest, DELETE (if enabled)
```

- `<repository>` is the full OCI repository name (e.g. `acme/widgets`), matching `/v2/{name}/…` path segment(s).
- `::rw` implies read for that repo (handler may treat `rw` as superset of `r`).

| Request | Claim needed |
|---------|----------------|
| `GET` / `HEAD` | `r` or `rw` on `{name}` |
| `POST` / `PATCH` / `PUT` / `DELETE` | `rw` on `{name}` |
| Mount `?from=` | `r` on source, `rw` on destination |

**Local / conformance**: `oci-registry-local` mints test JWTs with the same claim strings, or uses a test authorizer bypass flag — not unauthenticated production behavior.

---

## Per-client credential storage

| Client | How the Bearer token is supplied | Notes |
|--------|----------------------------------|--------|
| **OpenTofu** | [`oci_credentials`](https://opentofu.org/docs/cli/oci_registries/credentials/) in `.tofurc` | Preferred: `access_token = "<cognito-access-token>"`. Host label = registry hostname (e.g. `registry.example.com`). |
| **OpenTofu** | Ambient `~/.docker/config.json` | After `oras login` (see below). OpenTofu discovers Docker-style config automatically. |
| **oci-module** | `OCI_MODULE_TOKEN` or CLI `--token` | Sends `Authorization: Bearer` on distribution API calls. |
| **Infraweave CLI** | App auth session → Bearer (or Basic wrapper) | Align on Cognito access token as Bearer for registry host. |
| **CI / conformance** | Minted test JWT or secret env | Local `oci-registry-local` — same claim strings, not production Cognito. |

`TF_TOKEN_*` env vars apply to **OpenTofu protocol** registries only — **not** OCI `/v2/`.

---

## `oras login` and Cognito

[`oras login`](https://oras.land/docs/commands/oras_login) **does not talk to Cognito**. It writes credentials into a **Docker-style** config file so OCI-aware tools (including OpenTofu’s ambient credential discovery) can find them later.

| Step | What happens |
|------|----------------|
| Cognito | User/CI authenticates → **access token** (with custom claims) |
| Store token | `oci_credentials { access_token }`, `oras login … --password-stdin`, env/CLI |
| Registry call | Client sends **Bearer** on `GET/POST … /v2/…` |
| API Gateway | **Validates JWT** (signature, `exp`, `aud`) |
| Lambda | Checks repo claims vs `/v2/{name}/` |

**Typical operator flow:**

1. Obtain Cognito **access token** (with `infraweave_oci::…` claims via pre-token generation).
2. Persist for CLI tools, e.g.:

   ```bash
   echo "$COGNITO_ACCESS_TOKEN" | oras login registry.example.com -u oauth --password-stdin
   ```

3. Run `tofu init` or `oci-module push` with env/flags that send **Bearer**.

**Bearer vs Basic:** API Gateway’s **Cognito JWT authorizer** expects a **Bearer JWT** in `Authorization`. OpenTofu’s `oci_credentials { access_token = "…" }` sends Bearer directly. Tools using only Docker-style **Basic** may **not** satisfy a JWT authorizer — prefer `access_token` in `.tofurc` for OpenTofu.

---

## Azure

Dedicated APIM API or Functions custom domain; `/v2/*`; validate JWT in policy or middleware; same pre-provisioned Bearer rule for clients.

---

## Out of scope (MVP)

| Item | Why |
|------|-----|
| Distribution **token service** (`GET …/token?service=&scope=`) | Clients already hold Cognito JWT |
| **Docker** `WWW-Authenticate` challenge loop | Not a supported client |
| **Anonymous** pull | JWT required on all `/v2/*` including `GET`/`HEAD` |
| Registry issuing Cognito tokens | Cognito user pool + pre-token Lambda only |

HTTP API cannot emit spec-style `WWW-Authenticate: Bearer realm=…,scope=repository:…` on auth failure. Supported clients use pre-provisioned Bearer; JWT validation at the edge.

Optional later: registry returns OCI-shaped **401** + `realm=` pointing at a custom token bridge — only if a client cannot use pre-provisioned Bearer.
