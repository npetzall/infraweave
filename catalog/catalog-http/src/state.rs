use crate::error::{CatalogHttpErrorMap, DefaultCatalogHttpErrorMap};

fn management_auth_from_env() -> bool {
    std::env::var("CATALOG_HTTP_REQUIRE_AUTH_FOR_MANAGEMENT")
        .or_else(|_| std::env::var("CATALOG_AWS_APIGW_REQUIRE_AUTH_FOR_MANAGEMENT"))
        .or_else(|_| std::env::var("CATALOG_APIGW_REQUIRE_AUTH_FOR_MANAGEMENT"))
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Shared HTTP context: [`Catalog`] plus error mapping and management auth policy.
#[derive(Clone)]
pub struct AppState<C, E> {
    pub catalog: C,
    pub error_map: E,
    pub require_management_auth: bool,
}

impl<C, E: CatalogHttpErrorMap> AppState<C, E> {
    pub fn with_error_map(catalog: C, error_map: E) -> Self {
        Self {
            catalog,
            error_map,
            require_management_auth: management_auth_from_env(),
        }
    }

    pub fn map_err(&self, err: anyhow::Error) -> crate::error::ApiError {
        self.error_map.map_anyhow(err)
    }
}

impl<C> AppState<C, DefaultCatalogHttpErrorMap> {
    pub fn new(catalog: C) -> Self {
        Self::with_error_map(catalog, DefaultCatalogHttpErrorMap)
    }
}
