//! HTTP adaptor for the catalog trait surface (AWS Lambda + API Gateway HTTP API).
//!
//! Routing and handlers live in [`catalog_http`]. This crate adds the Lambda binary, optional
//! `catalog-aws` bootstrap, API Gateway identity, and AWS-specific error mapping. See crate
//! `README.md` for how to run and test.

#[cfg(all(feature = "aws", feature = "mem"))]
compile_error!("enable at most one of `aws` or `mem`");

#[cfg(feature = "aws")]
mod aws_error;

#[cfg(all(test, feature = "aws"))]
mod lambda_integration_test;

#[cfg(all(test, feature = "aws"))]
mod test_catalog;

pub mod identity;

use axum::Router;
use catalog_http::build_router as build_catalog_http_router;
use catalog_trait::Catalog;

pub use catalog_http::AppState;

#[cfg(feature = "aws")]
pub use aws_error::AwsCatalogHttpErrorMap;

/// Composes the catalog HTTP API with API Gateway identity extraction.
#[cfg(feature = "aws")]
pub fn build_router<C>(state: AppState<C, AwsCatalogHttpErrorMap>) -> Router
where
    C: Catalog + Clone + Send + Sync + 'static,
{
    build_catalog_http_router::<C, AwsCatalogHttpErrorMap, identity::ApiGatewayIdentity>(state)
}

/// Same routes as the AWS build, using the default error mapper (no `catalog-aws` downcast).
#[cfg(not(feature = "aws"))]
pub fn build_router<C>(state: AppState<C, catalog_http::DefaultCatalogHttpErrorMap>) -> Router
where
    C: Catalog + Clone + Send + Sync + 'static,
{
    build_catalog_http_router::<
        C,
        catalog_http::DefaultCatalogHttpErrorMap,
        identity::ApiGatewayIdentity,
    >(state)
}
