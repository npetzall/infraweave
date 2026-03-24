//! OpenAPI schema placeholders for JSON bodies defined outside this crate (`catalog` types).

use utoipa::ToSchema;

/// Paginated or single catalog entry JSON (shape matches `catalog_trait::read` serde types).
#[derive(ToSchema)]
#[schema(value_type = Object)]
pub struct CatalogJsonBody;
