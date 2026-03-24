use axum::extract::Extension;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use catalog_trait::Catalog;

use crate::error::CatalogHttpErrorMap;
use crate::state::AppState;

/// Health check for load balancers and synthetic monitoring.
#[utoipa::path(
    get,
    path = "/catalog/health",
    tag = "catalog",
    responses((status = 200, description = "Service is healthy"))
)]
pub async fn health<C, E>(Extension(_state): Extension<AppState<C, E>>) -> impl IntoResponse
where
    C: Catalog + Clone + Send + Sync + 'static,
    E: CatalogHttpErrorMap + Clone + Send + Sync + 'static,
{
    StatusCode::OK
}
