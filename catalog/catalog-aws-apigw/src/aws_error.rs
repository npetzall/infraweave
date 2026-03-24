//! AWS-specific mapping from [`catalog_aws::CatalogError`] to [`catalog_http::ApiError`].

use catalog_aws::CatalogError;
use catalog_http::{ApiError, CatalogHttpErrorMap};

/// Error mapper for Lambda + Dynamo-backed catalogs (downcasts to [`CatalogError`]).
#[derive(Clone, Copy, Debug, Default)]
pub struct AwsCatalogHttpErrorMap;

impl CatalogHttpErrorMap for AwsCatalogHttpErrorMap {
    fn map_anyhow(&self, err: anyhow::Error) -> ApiError {
        if let Some(ce) = err.downcast_ref::<CatalogError>() {
            return map_catalog_aws(ce);
        }
        tracing::error!(error = %err, "unhandled catalog error");
        ApiError::internal("internal error".to_string())
    }
}

fn map_catalog_aws(err: &CatalogError) -> ApiError {
    match err {
        CatalogError::NotFound { .. } => ApiError::not_found(err.to_string()),
        CatalogError::InvalidInput { message, .. } => ApiError::bad_request(message.clone()),
        CatalogError::Conflict { .. } => {
            tracing::warn!(error = %err, "catalog-aws conflict");
            ApiError::conflict(err.to_string())
        }
        CatalogError::Storage { .. } | CatalogError::Serialization { .. } => {
            tracing::error!(error = %err, "catalog-aws storage/serialization");
            ApiError::internal(err.to_string())
        }
    }
}
