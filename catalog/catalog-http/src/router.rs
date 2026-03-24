//! Axum route table: `/catalog/health` and `/catalog/v1/...`.

use axum::middleware::from_fn;
use axum::routing::{get, post};
use axum::Extension;
use axum::Router;
use catalog_trait::Catalog;

#[cfg(feature = "swagger-ui")]
use utoipa_swagger_ui::SwaggerUi;

use crate::error::CatalogHttpErrorMap;
use crate::handler::health;
use crate::identity::{inject_caller_identity, CallerIdentity};
use crate::management_handlers::{
    deprecate_module, deprecate_provider, deprecate_stack, promote_module, promote_provider,
    promote_stack, yank_module, yank_provider, yank_stack,
};
use crate::openapi::serve_openapi_json;
#[cfg(feature = "swagger-ui")]
use crate::openapi::ApiDoc;
use crate::read_handlers::{
    download_module_artifact, download_module_attachment, download_provider_artifact,
    download_provider_attachment, download_stack_artifact, download_stack_attachment,
    get_module_entry, get_provider_entry, get_stack_entry, list_module_attachments,
    list_module_versions, list_modules, list_provider_attachments, list_providers,
    list_stack_attachments, list_stack_versions, list_stacks,
};
use crate::state::AppState;
#[cfg(feature = "swagger-ui")]
use utoipa::OpenApi;

/// Stateless [`Router<()>`](axum::Router) so hosts (e.g. `lambda_http::run`) can use it without
/// catalog type parameters. Catalog and error mapping are supplied via [`Extension`] wrapping
/// [`AppState`].
pub fn build_router<C, E, I>(state: AppState<C, E>) -> Router
where
    C: Catalog + Clone + Send + Sync + 'static,
    E: CatalogHttpErrorMap + Clone + Send + Sync + 'static,
    I: CallerIdentity + 'static,
{
    let v1 = Router::new()
        .route("/provider/promote", post(promote_provider::<C, E, I>))
        .route("/provider/deprecate", post(deprecate_provider::<C, E, I>))
        .route("/provider/yank", post(yank_provider::<C, E, I>))
        .route("/module/promote", post(promote_module::<C, E, I>))
        .route("/module/deprecate", post(deprecate_module::<C, E, I>))
        .route("/module/yank", post(yank_module::<C, E, I>))
        .route("/stack/promote", post(promote_stack::<C, E, I>))
        .route("/stack/deprecate", post(deprecate_stack::<C, E, I>))
        .route("/stack/yank", post(yank_stack::<C, E, I>))
        .route("/providers", get(list_providers::<C, E>))
        .route("/modules", get(list_modules::<C, E>))
        .route("/stacks", get(list_stacks::<C, E>))
        .route(
            "/modules/versions/:track/:name",
            get(list_module_versions::<C, E>),
        )
        .route(
            "/stacks/versions/:track/:name",
            get(list_stack_versions::<C, E>),
        )
        .route(
            "/provider/:track/:name/:version/attachments/:attachment_name",
            get(download_provider_attachment::<C, E>),
        )
        .route(
            "/module/:track/:name/:version/attachments/:attachment_name",
            get(download_module_attachment::<C, E>),
        )
        .route(
            "/stack/:track/:name/:version/attachments/:attachment_name",
            get(download_stack_attachment::<C, E>),
        )
        .route(
            "/provider/:track/:name/:version/attachments",
            get(list_provider_attachments::<C, E>),
        )
        .route(
            "/module/:track/:name/:version/attachments",
            get(list_module_attachments::<C, E>),
        )
        .route(
            "/stack/:track/:name/:version/attachments",
            get(list_stack_attachments::<C, E>),
        )
        .route(
            "/provider/:track/:name/:version/download",
            get(download_provider_artifact::<C, E>),
        )
        .route(
            "/module/:track/:name/:version/download",
            get(download_module_artifact::<C, E>),
        )
        .route(
            "/stack/:track/:name/:version/download",
            get(download_stack_artifact::<C, E>),
        )
        .route(
            "/provider/:track/:name/:version",
            get(get_provider_entry::<C, E>),
        )
        .route(
            "/module/:track/:name/:version",
            get(get_module_entry::<C, E>),
        )
        .route("/stack/:track/:name/:version", get(get_stack_entry::<C, E>));

    let catalog = Router::new()
        .route("/health", get(health::<C, E>))
        .nest("/v1", v1);

    let app = Router::new()
        .route("/openapi.json", get(serve_openapi_json))
        .nest("/catalog", catalog);

    #[cfg(feature = "swagger-ui")]
    let app = app.merge(SwaggerUi::new("/swagger-ui").url("/openapi.json", ApiDoc::openapi()));

    app.layer(from_fn(inject_caller_identity::<I>))
        .layer(Extension(state))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::NoopIdentity;
    use crate::test_catalog::{PresetCatalog, StubCatalog};
    use axum::body::to_bytes;
    use axum::body::Body;
    use axum::extract::Extension;
    use axum::http::{Request, StatusCode};
    use axum::response::IntoResponse;
    use tower::ServiceExt;

    #[tokio::test]
    async fn health_returns_200_with_stub_catalog() {
        let response = health(Extension(AppState::new(StubCatalog)))
            .await
            .into_response();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn get_catalog_health_via_router_returns_200() {
        let app = build_router::<_, _, NoopIdentity>(AppState::new(StubCatalog));
        let req = Request::builder()
            .method("GET")
            .uri("/catalog/health")
            .body(Body::empty())
            .unwrap();
        let res = app.oneshot(req).await.expect("response");
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn legacy_root_health_not_routed() {
        let app = build_router::<_, _, NoopIdentity>(AppState::new(StubCatalog));
        let req = Request::builder()
            .method("GET")
            .uri("/health")
            .body(Body::empty())
            .unwrap();
        let res = app.oneshot(req).await.expect("response");
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn v1_routes_require_catalog_prefix() {
        let app = build_router::<_, _, NoopIdentity>(AppState::new(StubCatalog));
        let req = Request::builder()
            .method("GET")
            .uri("/v1/modules")
            .body(Body::empty())
            .unwrap();
        let res = app.oneshot(req).await.expect("response");
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn openapi_json_lists_catalog_paths() {
        let app = build_router::<_, _, NoopIdentity>(AppState::new(StubCatalog));
        let req = Request::builder()
            .method("GET")
            .uri("/openapi.json")
            .body(Body::empty())
            .unwrap();
        let res = app.oneshot(req).await.expect("response");
        assert_eq!(res.status(), StatusCode::OK);
        let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let paths = v["paths"].as_object().expect("paths object");
        assert!(paths.contains_key("/catalog/health"));
        assert!(paths.contains_key("/catalog/v1/modules"));
    }

    #[tokio::test]
    async fn post_promote_provider_returns_204() {
        let app = build_router::<_, _, NoopIdentity>(AppState::new(
            PresetCatalog::new().with_management_ok(),
        ));
        let body = r#"{"reference":{"id":"ref-1"},"track":"main","version":null}"#;
        let req = Request::builder()
            .method("POST")
            .uri("/catalog/v1/provider/promote")
            .header("content-type", "application/json")
            .body(Body::from(body))
            .unwrap();
        let res = app.oneshot(req).await.expect("response");
        assert_eq!(res.status(), StatusCode::NO_CONTENT);
    }
}
