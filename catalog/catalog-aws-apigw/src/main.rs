//! Lambda entrypoint (API Gateway HTTP API → Axum via `lambda_http`).
//!
//! [`lambda_http::run`] wraps [`lambda_runtime::run`] and converts each proxy event into an
//! `http::Request` (method, stage-normalized path, query string, headers, base64-decoded body)
//! before invoking the Axum [`Router`](axum::Router). See `catalog-aws-apigw/README.md` (Phase 3).
//!
//! With default features, boots [`catalog_aws::AwsCatalog`] from the environment.
//! With `--no-default-features --features mem`, boots [`catalog_mem::MemCatalog`].
//! Without `aws` or `mem`, this binary exits with instructions (library tests use `--no-default-features`).

#[cfg(all(feature = "aws", feature = "mem"))]
compile_error!("enable at most one of `aws` or `mem`");

#[cfg(all(feature = "aws", not(feature = "mem")))]
mod bootstrap_aws;

#[cfg(all(feature = "mem", not(feature = "aws")))]
mod bootstrap_mem;

#[cfg(all(feature = "aws", not(feature = "mem")))]
use catalog_aws_apigw::{build_router, AppState, AwsCatalogHttpErrorMap};

#[cfg(all(feature = "mem", not(feature = "aws")))]
use catalog_aws_apigw::{build_router, AppState};

#[cfg(any(
    all(feature = "aws", not(feature = "mem")),
    all(feature = "mem", not(feature = "aws"))
))]
fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .try_init();
}

#[cfg(all(feature = "aws", not(feature = "mem")))]
#[tokio::main]
async fn main() -> Result<(), lambda_http::Error> {
    init_tracing();

    let catalog = bootstrap_aws::init_catalog()
        .await
        .map_err(|e| lambda_http::Error::from(format!("catalog init: {e:#}")))?;

    let app = build_router(AppState::with_error_map(catalog, AwsCatalogHttpErrorMap));
    lambda_http::run(app).await
}

#[cfg(all(feature = "mem", not(feature = "aws")))]
#[tokio::main]
async fn main() -> Result<(), lambda_http::Error> {
    init_tracing();

    let catalog = bootstrap_mem::init_catalog()
        .map_err(|e| lambda_http::Error::from(format!("catalog init: {e:#}")))?;

    let app = build_router(AppState::new(catalog));
    lambda_http::run(app).await
}

#[cfg(not(any(feature = "aws", feature = "mem")))]
fn main() {
    eprintln!(
        "catalog-aws-apigw Lambda binary requires the `aws` or `mem` feature (default is `aws`)."
    );
    eprintln!("Examples:");
    eprintln!("  cargo run -p catalog-aws-apigw --bin bootstrap");
    eprintln!(
        "  cargo run -p catalog-aws-apigw --no-default-features --features mem --bin bootstrap"
    );
    eprintln!("  cargo lambda watch  # https://www.cargo-lambda.info");
    eprintln!("Tests without linking catalog-aws:");
    eprintln!("  cargo test -p catalog-aws-apigw --no-default-features");
    std::process::exit(1);
}
