# catalog-aws-apigw

HTTP edge for the [`catalog-trait`](../catalog-trait) trait surface, intended to run on **AWS Lambda** behind **API Gateway HTTP API** (v2). Enable **`aws`** or **`mem`** to link a catalog backend and `lambda_http` for the `bootstrap` binary (`aws` is the default). Do not enable both `aws` and `mem` at once (the build fails with a clear error).

## Features

| Feature | Effect |
|--------|--------|
| `aws` (default) | Depends on `catalog-aws` and `lambda_http`; Lambda binary `bootstrap` loads `AwsCatalog::from_env()` ([`catalog-aws`](../catalog-aws)) and serves Axum. |
| `mem` | Depends on `catalog-mem` and `lambda_http`; `bootstrap` uses an empty [`MemCatalog`](../catalog-mem) (`MemCatalog::default()`). Typical invocation: `--no-default-features --features mem`. |
| *(none)* | Library only: no `catalog-aws`, `catalog-mem`, or `lambda_http`. The `bootstrap` binary prints a short message and exits non-zero. |

Environment variables for the AWS catalog are defined by **`catalog-aws::Config`** (see that crate’s docs and `ai-task/catalog-aws/` notes)—this crate does not redefine them.

## Build and test

```bash
# Production-style build (default features = aws)
cargo build -p catalog-aws-apigw

# Fast tests: stub `Catalog`, no AWS SDK / catalog-aws in the dependency graph
cargo test -p catalog-aws-apigw --no-default-features

# Default tests additionally run API Gateway v2 → Axum roundtrips (see Phase 3)
cargo test -p catalog-aws-apigw

# Shared HTTP router (`catalog-http`): Axum oneshot tests for `/catalog/...` routes
cargo test -p catalog-http
```

For **Linux / Lambda** targets (`x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`), build in an environment that provides the usual native toolchain for AWS SDK dependencies (for example Linux CI, Docker, or `cargo-lambda`’s build images). Cross-compiling from macOS without a Linux linker will fail at crates that use `cc` (for example `ring`).

## Run locally

With default features, cold start loads AWS config and DynamoDB/S3 clients from the environment (same as `catalog-aws`):

```bash
cargo run -p catalog-aws-apigw --bin bootstrap
```

In-memory catalog (no AWS SDK):

```bash
cargo run -p catalog-aws-apigw --no-default-features --features mem --bin bootstrap
```

For iterative Lambda-style runs, use [cargo-lambda](https://www.cargo-lambda.info) (for example `cargo lambda watch --bin bootstrap`).

## Lambda handler

The binary name is **`bootstrap`**. Configure the Lambda **Handler** or bootstrap command to run this executable per your deployment tool (SAM, Terraform, CDK, etc.).

## Lambda + API Gateway HTTP API (Phase 3)

**Integration approach:** **Option A** from `ai-task/catalog-aws-apigw/phase-3.md` — [`lambda_http`](https://docs.rs/lambda_http) + Axum. The binary calls [`lambda_http::run`](https://docs.rs/lambda_http/latest/lambda_http/fn.run.html), which uses `lambda_runtime` internally and maps **payload format 2.0** (HTTP API) to `http` types. Option B (manual JSON → `axum::http::Request`, as in `internal-api/src/main_aws_unified.rs`) remains a documented fallback if this stack ever proves incompatible with a specific deployment.

**Stage and path:** `lambda_http` adjusts the request URI when the stage is not `$default` (it prefixes `/{stage}` when API Gateway sends `rawPath` without the stage). If you need the raw path only, set environment variable **`AWS_LAMBDA_HTTP_IGNORE_STAGE_IN_PATH`** (see the `lambda_http` crate). Route definitions use the path Axum sees after that normalization (for `$default`, this matches `rawPath`).

**Bodies:** API Gateway sets `isBase64Encoded` for binary uploads; `lambda_http` decodes before the request reaches Axum. HTTP API has a **~6 MB** payload limit, which matters for publish-style bodies (Phase 5); read-only traffic (Phase 4) is usually well below that.

**Compression / `Content-Encoding`:** As noted in `internal-api` (`http_router.rs`), API Gateway HTTP API may strip or mishandle `Content-Encoding` on the way out. Prefer **uncompressed JSON** for metadata and **presigned S3 URLs** (or similar) for large blobs rather than gzip-through-API-GW.

**CORS:** If browsers call this API directly, configure CORS on the API Gateway route or add response headers in Lambda. Server-to-server callers can omit CORS.

**Identity (stub):** With the `aws` feature, the `identity` module (`src/identity.rs`) exposes `ApiGatewayIdentity` from `requestContext.authorizer` (JWT / IAM / Lambda authorizer JSON). Handlers do not enforce policy yet; call sites can thread this type in later phases.

### Example API Gateway HTTP API (v2) proxy event — `GET /catalog/health`

Trimmed input (full fixtures used in tests live under `catalog-aws-apigw/tests/fixtures/`):

```json
{
  "version": "2.0",
  "routeKey": "$default",
  "rawPath": "/catalog/health",
  "requestContext": {
    "accountId": "123456789012",
    "apiId": "api-id",
    "requestId": "req-health-1",
    "stage": "$default",
    "http": {
      "method": "GET",
      "path": "/catalog/health",
      "protocol": "HTTP/1.1",
      "sourceIp": "203.0.113.1",
      "userAgent": "curl/8.0"
    }
  },
  "isBase64Encoded": false
}
```

### Example Lambda proxy integration response — success (`GET /catalog/health`)

After `lambda_http` maps the Axum response, API Gateway receives a payload shaped like (exact headers depend on the handler):

```json
{
  "statusCode": 200,
  "headers": {
    "content-length": "0"
  },
  "body": ""
}
```

### Example — unmatched route (Axum 404)

For a request whose path has no matching route (for example `GET /catalog/v1/no-such-route`), Axum returns **404**. The integration response is along the lines of:

```json
{
  "statusCode": 404,
  "headers": {
    "content-type": "text/plain; charset=utf-8"
  },
  "body": ""
}
```

**End-to-end checks:** Run `cargo lambda watch --bin bootstrap` or deploy behind API Gateway and call `GET /catalog/health`. Unit tests deserialize real v2 JSON fixtures and drive the same `Router` as production (`cargo test -p catalog-aws-apigw`).

## Axum state vs `lambda_http`

`lambda_http::run` expects an Axum [`Router<()>`](https://docs.rs/axum). Shared catalog context is therefore injected with [`Extension<AppState<C>>`](https://docs.rs/axum/latest/axum/extract/struct.Extension.html) rather than router [`State`](https://docs.rs/axum/latest/axum/extract/struct.State.html). Handlers still receive a typed `AppState<C>` extractor. A future phase may switch to manual API Gateway → Axum bridging (as in `internal-api`) if `State` becomes necessary.

## HTTP API contract (Phase 2 + Phase 4)

The public HTTP surface lives under **`/catalog`**: health at **`/catalog/health`**, and the versioned API under **`/catalog/v1/...`**. It maps directly to [`catalog_trait::read::CatalogRead`](../catalog-trait/src/read.rs) (list / get / download / attachments). Route handlers live in the shared [`catalog-http`](../catalog-http) crate; this README summarizes behavior for operators.

**Canonical implementation:** [`catalog-http`](../catalog-http) (`build_router`), composed here with AWS identity and error mapping.

### Routes (summary)

| Purpose | Method and path |
|--------|------------------|
| List | `GET /catalog/v1/providers`, `GET /catalog/v1/modules`, `GET /catalog/v1/stacks` |
| List versions (aligned with internal-api) | `GET /catalog/v1/modules/versions/:track/:name`, `GET /catalog/v1/stacks/versions/:track/:name` |
| Get one entry | `GET /catalog/v1/provider|module|stack/:track/:name/:version` |
| Artifact location | `GET .../:version/download` |
| Attachments | `GET .../:version/attachments`, `GET .../:version/attachments/:attachment_name` |

`:version` is `latest` (case-insensitive) or an exact version string → [`VersionSelector`](../catalog-trait/src/types.rs).

### List query parameters

Aligned with [`catalog_trait::read::Query`](../catalog-trait/src/read.rs): `name`, `track`, `limit`, **`next`** (opaque continuation; with the `aws` backend this is the token from `catalog-aws` / `Page.next`), and **`projection`** (comma-separated: `metadata`, `manifest`, `terraform`, `version_diff`, `stack_data`; omit for full).

List responses use the same JSON shape as `Page<T>`: `{ "items": [...], "next": "<optional>" }`.

### Download behavior

Default: **`200`** JSON `{ "url": "..." }` for `ContentSource::Url`. Optional **`?redirect=1`**: **`302`** to that URL. `Bytes` → raw response when returned by the implementation; `Path` is not expected in Lambda-first deployments (see spec).

### Errors

JSON body:

```json
{ "error": { "code": "NOT_FOUND", "message": "...", "details": null } }
```

Typical mapping: **400** `BAD_REQUEST`, **404** `NOT_FOUND`, **500** `INTERNAL_ERROR`. AWS-specific mapping stays in this crate’s error adapter when the `aws` feature is enabled.
Management (Phase 5) when auth is required: **401** `UNAUTHORIZED` if neither authorizer context nor `Authorization` is present; **`catalog-aws`** maps `CatalogError::Conflict` to **409** `CONFLICT`.

### Management API (Phase 5)

**`POST`** routes under **`/catalog/v1/...`** call [`CatalogManagement`](../catalog-trait/src/management.rs): `promote`, `deprecate`, `yank` for each of `provider`, `module`, `stack`. **Success:** **204 No Content**. **Populate** (`add_*`, attachment uploads) is intentionally not implemented here; use presigned S3 or internal tooling.

Set **`CATALOG_HTTP_REQUIRE_AUTH_FOR_MANAGEMENT=true`** (or `1`) in production so callers must present API Gateway authorizer output or an `Authorization` header. For deployments that prefer an AWS APIGW–scoped name, **`CATALOG_AWS_APIGW_REQUIRE_AUTH_FOR_MANAGEMENT`** is equivalent. The older name **`CATALOG_APIGW_REQUIRE_AUTH_FOR_MANAGEMENT`** is still read for compatibility.
