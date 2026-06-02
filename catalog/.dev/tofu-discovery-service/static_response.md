# tofu-discovery-service — static response configuration

**Naming**: Folder and edge integration name `tofu-discovery-service` signals **OpenTofu** service discovery (not a HashiCorp Terraform Registry product). The on-the-wire path remains `/.well-known/terraform.json` per the OpenTofu/Terraform discovery spec.

OpenTofu **service discovery** is a single read-only document at the **host root**:

```http
GET /.well-known/terraform.json
```

There is **no `tofu-discovery-service` Rust crate** and no Lambda required for v1. The document is fixed JSON; clients resolve relative `providers.v1` / `modules.v1` paths against the discovery URL (see [`registry-client`](../../registry-client) `Registry::fetch_service_discovery`).

**Related docs**

| Doc | Topic |
|-----|--------|
| [`../registry/registry_apigw_routing.md`](../registry/registry_apigw_routing.md) | Path precedence on a shared API hostname |
| [`../registry/registry_edge_cases.md`](../registry/registry_edge_cases.md) | Discovery compliance edge cases |
| [`../registry/registry_decisions.md`](../registry/registry_decisions.md) | DEC-005, DEC-006 (`login.v1` omitted) |

---

## Canonical response body

Save this as `terraform.json` (or embed verbatim in gateway config):

```json
{
  "modules.v1": "/catalog/v1/modules/",
  "providers.v1": "/catalog/v1/providers/"
}
```

| Requirement | Value |
|-------------|--------|
| HTTP status | `200` |
| `Content-Type` | `application/json` (include charset if your platform adds `; charset=utf-8`) |
| Method | `GET` only for normal clients |
| `login.v1` | **Omit** in v1 (DEC-006) |
| Path bases | **Relative** paths under `/catalog/v1/` — clients join them to the discovery URL’s origin |

After discovery, clients call:

- `https://{host}/catalog/v1/providers/…`
- `https://{host}/catalog/v1/modules/…`

Route those prefixes to **tofu-provider-registry** / **tofu-module-registry** Lambdas (or local stand-ins), not to the discovery integration.

---

## Routing rule (all environments)

Register discovery **before** greedy or catch-all routes:

```text
GET /.well-known/terraform.json   →  static / mock response
/catalog/v1/providers/*           →  tofu-provider-registry
/catalog/v1/modules/*             →  tofu-module-registry
```

A greedy `/{proxy+}` or `/{namespace}/…` rule must **not** capture `/.well-known/terraform.json`.

---

## AWS (API Gateway HTTP API)

### Option A — Mock integration (recommended for v1)

Use a **Mock** integration on a dedicated route. No Lambda, no cold start.

**Route**

- Method: `GET`
- Path: `/.well-known/terraform.json` (exact; not parameterized)

**Integration (HTTP API v2)**

- Integration type: `MOCK`
- Response:
  - Status: `200`
  - `contentHandling`: leave default (no conversion)
- **Route response** / parameter mapping: set header `Content-Type` = `application/json`
- Body: the canonical JSON above (minified is fine)

**CDK sketch** (illustrative):

```typescript
const discovery = api.addRoutes({
  path: "/.well-known/terraform.json",
  methods: [apigatewayv2.HttpMethod.GET],
  integration: new integrations.HttpUrlIntegration(
    "DiscoveryMock",
    "https://example.invalid", // unused for MOCK; use MockIntegration in lower-level APIs
  ),
});
```

With L1/L2, prefer `AwsIntegration` / `MockIntegration` from `aws-apigatewayv2-integrations` so the body is inline in CloudFormation rather than a dummy URL.

**Terraform sketch** (HTTP API):

```hcl
resource "aws_apigatewayv2_route" "discovery" {
  api_id    = aws_apigatewayv2_api.catalog.id
  route_key = "GET /.well-known/terraform.json"
  target    = "integrations/${aws_apigatewayv2_integration.discovery.id}"
}

resource "aws_apigatewayv2_integration" "discovery" {
  api_id           = aws_apigatewayv2_api.catalog.id
  integration_type = "MOCK"

  request_templates = {
    "application/json" = jsonencode({ statusCode = 200 })
  }
}

resource "aws_apigatewayv2_route_response" "discovery" {
  api_id             = aws_apigatewayv2_api.catalog.id
  route_id           = aws_apigatewayv2_route.discovery.id
  route_response_key = "$default"
}

# Map integration response → JSON body + Content-Type via apigatewayv2_integration_response
```

Exact resource names vary by module version; the invariant is: **MOCK + 200 + JSON body + `Content-Type: application/json`**.

### Option B — S3 + CloudFront (or S3 website)

1. Upload `terraform.json` to S3 with `Content-Type: application/json`.
2. CloudFront behavior or ALB path rule: `/.well-known/terraform.json` → that object.
3. API Gateway **HTTP proxy** or separate behavior on the same custom domain.

Use when discovery is already served from a static-assets stack or you want IaC-owned object versioning without API Gateway body limits.

### Option C — Lambda (avoid for v1)

A one-line Lambda works but adds cold start and deploy surface. Reserve for **per-tenant** or **dynamic** discovery later.

### AWS checklist

- [ ] Custom domain uses **HTTPS** (ACM cert on API Gateway / CloudFront).
- [ ] Discovery route registered **above** `/catalog/{proxy+}` routes.
- [ ] Response is **uncompressed** JSON, or compression sets correct `Content-Encoding`.
- [ ] No auth on discovery unless product explicitly requires it (OpenTofu public registries are anonymous for discovery).
- [ ] CDN cache TTL: short or bypass for discovery if you change `/catalog/v1/` paths often (see edge cases).

---

## Azure

Equivalent patterns on **API Management (APIM)** or **Application Gateway + static storage**.

### Option A — APIM mock / return-static (recommended for v1)

**Inbound policy** on operation `GET /.well-known/terraform.json`:

```xml
<policies>
  <inbound>
    <base />
    <return-response>
      <set-status code="200" reason="OK" />
      <set-header name="Content-Type" exists-action="override">
        <value>application/json</value>
      </set-header>
      <set-body>@{
        return "{\"modules.v1\":\"/catalog/v1/modules/\",\"providers.v1\":\"/catalog/v1/providers/\"}";
      }</set-body>
    </return-response>
  </inbound>
  <backend><base /></backend>
  <outbound><base /></outbound>
</policies>
```

No backend pool required for that operation if `return-response` short-circuits in inbound.

Alternatively store the JSON in APIM **named value** / **fragment** and reference it from `set-body` for easier edits.

### Option B — Static file on Blob Storage

1. Upload `terraform.json` to a container with `Content-Type: application/json`.
2. **Azure Front Door** or **Application Gateway** path rule: `/.well-known/terraform.json` → blob URL or origin path.
3. APIM can proxy the same path to blob for a single front door.

### Option C — Azure Functions (avoid for v1)

HTTP trigger returning fixed JSON — same trade-off as Lambda.

### Azure checklist

- [ ] Same path map as AWS: discovery at host root; `/catalog/v1/providers/*` and `/catalog/v1/modules/*` to Functions.
- [ ] TLS terminated at Front Door / APIM with valid cert for customer hostname.
- [ ] APIM **path prefix** rules do not swallow `/.well-known/*` before the discovery operation.
- [ ] If using APIM in front of multiple backends, discovery operation does not require JWT (DEC-006 / extension auth is on publish routes only).

---

## Local development & mocking

[`registry-client`](../../registry-client) treats non-builtin hosts as discovery-first. Builtin hosts (`registry.terraform.io`, `registry.opentofu.org`) skip `/.well-known` and use `/v1/providers` and `/v1/modules` directly.

### Loopback HTTP (tests and local CLI)

For hosts like `127.0.0.1:PORT` or `localhost:PORT`, the client uses **HTTP** (not HTTPS) for discovery:

```text
http://127.0.0.1:9/.well-known/terraform.json
```

Production edge should still use HTTPS; loopback is for dev/tests only.

### Option 1 — Wiremock (unit / integration tests)

Used by `registry-client` tests: stub

```text
GET /.well-known/terraform.json
→ 200, Content-Type: application/json, body = canonical JSON
```

Record/replay fixtures when testing full init flows against a fake catalog host.

### Option 2 — Minimal Axum (local catalog stack)

When running **tofu-provider-registry-local** / **tofu-module-registry-local** behind one Axum app, add a single route:

```rust
// Conceptual — colocate with local gateway binary or test harness
.get("/.well-known/terraform.json", || async {
    (
        [(header::CONTENT_TYPE, "application/json")],
        r#"{"modules.v1":"/catalog/v1/modules/","providers.v1":"/catalog/v1/providers/"}"#,
    )
})
```

Mount **before** nested `/catalog/v1/...` routers. Same process or reverse proxy on one port is enough for `tofu init` against `127.0.0.1:PORT`.

### Option 3 — Reverse proxy (docker-compose / dev)

**Caddy** example:

```caddyfile
:8080 {
  handle /.well-known/terraform.json {
    header Content-Type application/json
    respond `{"modules.v1":"/catalog/v1/modules/","providers.v1":"/catalog/v1/providers/"}`
  }
  handle /catalog/v1/providers/* {
    reverse_proxy tofu-provider-registry:8081
  }
  handle /catalog/v1/modules/* {
    reverse_proxy tofu-module-registry:8082
  }
}
```

**nginx** `location = /.well-known/terraform.json` with `default_type application/json` and `return 200 '…';` works similarly.

### Option 4 — Mock APIGW locally

There is no official “API Gateway on laptop” for HTTP API v2. For local dev, prefer Axum or Caddy above rather than emulating AWS Mock integrations.

### Local checklist

- [ ] Discovery and protocol routes share the **same Host** header the user puts in `tofu` / module source (`registry.example.com` or `127.0.0.1:PORT`).
- [ ] Relative paths in JSON resolve to `http://127.0.0.1:PORT/catalog/v1/...` in tests.
- [ ] Wiremock or Axum returns **`application/json`**; avoid `text/plain` or missing content type.
- [ ] Do not redirect discovery to `/catalog/...` unless you intend clients to follow redirects and re-resolve bases.

---

## When static configuration is not enough

Move to dynamic discovery (Lambda, APIM backend, or per-host config) only if you need:

- Different `modules.v1` / `providers.v1` per **hostname** (see [`../tofu-module-registry/open_questions.md`](../tofu-module-registry/open_questions.md) OQ-015)
- Per-tenant discovery on one host
- `login.v1` / OAuth (out of v1 scope — DEC-006)

Until then, **edge static JSON + `registry-client` discovery parsing** is the full story.

---

## Verification

```bash
# Production-style (replace host)
curl -sS -D - "https://catalog.example.com/.well-known/terraform.json"

# Local
curl -sS -D - "http://127.0.0.1:8080/.well-known/terraform.json"
```

Expect:

```http
HTTP/1.1 200 OK
Content-Type: application/json
...
{"modules.v1":"/catalog/v1/modules/","providers.v1":"/catalog/v1/providers/"}
```

Then confirm bases resolve:

```bash
curl -sS -o /dev/null -w "%{http_code}\n" \
  "https://catalog.example.com/catalog/v1/providers/hashicorp/random/versions"
```

(404/401 is fine for an empty cache; connection errors or wrong path prefix mean routing failed.)

**Rust**: run `registry-client` discovery tests / wiremock harness after changing the document shape.
