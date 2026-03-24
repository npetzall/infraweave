//! Axum HTTP router over the [`catalog`] traits.
//!
//! Exposes [`build_router`] for `/catalog/health`, the versioned API under `/catalog/v1`, and
//! [`serve_openapi_json`] at **`/openapi.json`**. Enable the **`swagger-ui`** feature for
//! **`/swagger-ui`** (embedded Swagger UI).

#![forbid(unsafe_code)]

mod error;
mod handler;
mod identity;
mod management_handlers;
mod openapi;
mod openapi_types;
mod read_handlers;
mod router;
mod state;

#[cfg(test)]
mod test_catalog;

pub use error::{
    ApiError, CatalogHttpErrorMap, DefaultCatalogHttpErrorMap, ErrorBody, ErrorPayload,
};
pub use identity::{inject_caller_identity, CallerIdentity, NoopIdentity};
pub use openapi::{openapi_json_value, serve_openapi_json, ApiDoc};
pub use router::build_router;
pub use state::AppState;
