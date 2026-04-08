# catalog-aws-provider-mirror

Thin AWS Lambda binary that receives async [`InvokeFunction`](https://docs.aws.amazon.com/lambda/latest/api/API_Invoke.html) payloads (`Event` type) from the catalog API after a successful module/stack/provider publish. It mirrors Terraform/OpenTofu registry artifacts into the catalog **providers** S3 bucket by calling [`catalog_aws::mirror::mirror_tf_lock_providers`](https://github.com/npetzall/infraweave/tree/main/catalog/catalog-aws) — no duplicated mirror logic.

## Required environment (worker)

- **`AWS_REGION`** — AWS region.
- **`CATALOG_PROVIDER_MIRROR_BUCKET`** — S3 bucket for mirrored Terraform provider artifacts (distinct from module/provider catalog buckets; default `providers` only for local dev).

Mirror-specific:

- **`REGISTRY_API_HOSTNAME`** — registry API host (default `registry.opentofu.org`, same as `utils/terraform.rs`).
- **`CATALOG_PROVIDER_MIRROR_PLATFORMS`** — comma-separated Terraform-style platform strings, e.g. `linux_amd64,linux_arm64` (default: `linux_amd64,linux_arm64`).

Do **not** set **`CATALOG_PROVIDER_MIRROR_ARN`** on this function unless you intend to build a Lambda client for nested invokes; the worker only needs S3.

## Local / test stack

When `TEST_MODE` or `DYNAMODB_ENDPOINT` is set, `catalog-aws` enters local mode and expects DynamoDB and S3 endpoint env vars (same as `catalog-aws-apigw`). Use that only for integrated local testing.

## Sample invoke payload

JSON matching `MirrorInvokePayload` in `catalog-aws` (`catalog/catalog-aws/src/mirror/payload.rs`):

```json
{
  "correlation_id": "optional-trace-id",
  "providers": [
    { "source": "registry.opentofu.org/hashicorp/aws", "version": "5.0.0" }
  ]
}
```

## Build

```bash
cargo build -p catalog-aws-provider-mirror --bin bootstrap
cargo test -p catalog-aws-provider-mirror
```

Deployment (timeout, IAM, second Lambda) is covered in Phase 5 of the provider mirror plan under `ai-task/catalog-aws/`.
