//! API Gateway HTTP API (v2) → Axum roundtrip tests (`lambda_http` request adapter).

use axum::http::StatusCode;
use lambda_http::request::from_str;
use tower::ServiceExt;

use crate::test_catalog::StubCatalog;
use crate::{build_router, identity::identity_from_request, AppState, AwsCatalogHttpErrorMap};

const GET_HEALTH: &str = include_str!("../tests/fixtures/apigw_v2_get_health.json");
const GET_UNKNOWN: &str = include_str!("../tests/fixtures/apigw_v2_get_unknown.json");

#[tokio::test]
async fn apigw_v2_get_health_maps_to_axum_200() {
    let req = from_str(GET_HEALTH).expect("fixture parses as API Gateway v2 event");
    assert_eq!(req.method(), "GET");
    assert_eq!(req.uri().path(), "/catalog/health");

    let id = identity_from_request(&req);
    assert!(
        id.authorizer_json
            .as_ref()
            .is_some_and(|s| s.contains("claim1")),
        "fixture includes JWT authorizer; identity hook should capture it"
    );

    let app = build_router(AppState::with_error_map(
        StubCatalog,
        AwsCatalogHttpErrorMap,
    ));
    let response = app.oneshot(req).await.expect("handler ok");

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn apigw_v2_unknown_route_returns_404() {
    let req = from_str(GET_UNKNOWN).expect("fixture parses");
    let app = build_router(AppState::with_error_map(
        StubCatalog,
        AwsCatalogHttpErrorMap,
    ));
    let response = app.oneshot(req).await.expect("handler ok");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
