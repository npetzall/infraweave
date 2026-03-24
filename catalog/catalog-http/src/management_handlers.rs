//! [`catalog_trait::CatalogManagement`] over HTTP (`POST` JSON bodies).

use axum::http::header;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::{Extension, Json};
use catalog_trait::types::{CatalogKind, CatalogRef};
use catalog_trait::Catalog;
use serde::Deserialize;
use utoipa::ToSchema;

use crate::error::{ApiError, CatalogHttpErrorMap};
use crate::identity::CallerIdentity;
use crate::state::AppState;

#[derive(Clone, Debug, Deserialize, ToSchema)]
pub struct CatalogRefWire {
    pub id: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct PromoteBody {
    pub reference: CatalogRefWire,
    pub track: String,
    pub version: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct DeprecateBody {
    pub reference: CatalogRefWire,
    pub reason: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct YankBody {
    pub reference: CatalogRefWire,
}

fn require_management_auth<C, E, I>(
    state: &AppState<C, E>,
    headers: &HeaderMap,
    identity: &I,
) -> Result<(), ApiError>
where
    I: CallerIdentity,
{
    if !state.require_management_auth {
        return Ok(());
    }
    if identity.has_authorizer_context() {
        return Ok(());
    }
    if headers.get(header::AUTHORIZATION).is_some() {
        return Ok(());
    }
    Err(ApiError::unauthorized(
        "authentication required for management routes (authorizer context or Authorization header)",
    ))
}

fn ref_from_wire(w: CatalogRefWire) -> CatalogRef {
    CatalogRef { id: w.id }
}

#[utoipa::path(
    post,
    path = "/catalog/v1/provider/promote",
    tag = "catalog",
    request_body = PromoteBody,
    responses(
        (status = 204, description = "Provider promoted"),
        (status = 401, description = "Management authentication required", body = crate::error::ErrorBody),
        (status = 400, description = "Bad request", body = crate::error::ErrorBody),
        (status = 409, description = "Conflict", body = crate::error::ErrorBody),
        (status = 500, description = "Internal error", body = crate::error::ErrorBody)
    )
)]
pub async fn promote_provider<C, E, I>(
    Extension(state): Extension<AppState<C, E>>,
    Extension(identity): Extension<I>,
    headers: HeaderMap,
    Json(body): Json<PromoteBody>,
) -> Result<impl IntoResponse, ApiError>
where
    C: Catalog + Clone + Send + Sync + 'static,
    E: CatalogHttpErrorMap + Clone + Send + Sync + 'static,
    I: CallerIdentity,
{
    promote_inner(&state, &headers, &identity, CatalogKind::Provider, body).await
}

#[utoipa::path(
    post,
    path = "/catalog/v1/module/promote",
    tag = "catalog",
    request_body = PromoteBody,
    responses(
        (status = 204, description = "Module promoted"),
        (status = 401, description = "Management authentication required", body = crate::error::ErrorBody),
        (status = 400, description = "Bad request", body = crate::error::ErrorBody),
        (status = 409, description = "Conflict", body = crate::error::ErrorBody),
        (status = 500, description = "Internal error", body = crate::error::ErrorBody)
    )
)]
pub async fn promote_module<C, E, I>(
    Extension(state): Extension<AppState<C, E>>,
    Extension(identity): Extension<I>,
    headers: HeaderMap,
    Json(body): Json<PromoteBody>,
) -> Result<impl IntoResponse, ApiError>
where
    C: Catalog + Clone + Send + Sync + 'static,
    E: CatalogHttpErrorMap + Clone + Send + Sync + 'static,
    I: CallerIdentity,
{
    promote_inner(&state, &headers, &identity, CatalogKind::Module, body).await
}

#[utoipa::path(
    post,
    path = "/catalog/v1/stack/promote",
    tag = "catalog",
    request_body = PromoteBody,
    responses(
        (status = 204, description = "Stack promoted"),
        (status = 401, description = "Management authentication required", body = crate::error::ErrorBody),
        (status = 400, description = "Bad request", body = crate::error::ErrorBody),
        (status = 409, description = "Conflict", body = crate::error::ErrorBody),
        (status = 500, description = "Internal error", body = crate::error::ErrorBody)
    )
)]
pub async fn promote_stack<C, E, I>(
    Extension(state): Extension<AppState<C, E>>,
    Extension(identity): Extension<I>,
    headers: HeaderMap,
    Json(body): Json<PromoteBody>,
) -> Result<impl IntoResponse, ApiError>
where
    C: Catalog + Clone + Send + Sync + 'static,
    E: CatalogHttpErrorMap + Clone + Send + Sync + 'static,
    I: CallerIdentity,
{
    promote_inner(&state, &headers, &identity, CatalogKind::Stack, body).await
}

async fn promote_inner<C, E, I>(
    state: &AppState<C, E>,
    headers: &HeaderMap,
    identity: &I,
    kind: CatalogKind,
    body: PromoteBody,
) -> Result<impl IntoResponse, ApiError>
where
    C: Catalog,
    E: CatalogHttpErrorMap,
    I: CallerIdentity,
{
    require_management_auth(state, headers, identity)?;
    let reference = ref_from_wire(body.reference);
    state
        .catalog
        .promote(kind, &reference, &body.track, body.version.as_deref())
        .await
        .map_err(|e| state.map_err(e))?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/catalog/v1/provider/deprecate",
    tag = "catalog",
    request_body = DeprecateBody,
    responses(
        (status = 204, description = "Provider deprecated"),
        (status = 401, description = "Management authentication required", body = crate::error::ErrorBody),
        (status = 400, description = "Bad request", body = crate::error::ErrorBody),
        (status = 409, description = "Conflict", body = crate::error::ErrorBody),
        (status = 500, description = "Internal error", body = crate::error::ErrorBody)
    )
)]
pub async fn deprecate_provider<C, E, I>(
    Extension(state): Extension<AppState<C, E>>,
    Extension(identity): Extension<I>,
    headers: HeaderMap,
    Json(body): Json<DeprecateBody>,
) -> Result<impl IntoResponse, ApiError>
where
    C: Catalog + Clone + Send + Sync + 'static,
    E: CatalogHttpErrorMap + Clone + Send + Sync + 'static,
    I: CallerIdentity,
{
    deprecate_inner(&state, &headers, &identity, CatalogKind::Provider, body).await
}

#[utoipa::path(
    post,
    path = "/catalog/v1/module/deprecate",
    tag = "catalog",
    request_body = DeprecateBody,
    responses(
        (status = 204, description = "Module deprecated"),
        (status = 401, description = "Management authentication required", body = crate::error::ErrorBody),
        (status = 400, description = "Bad request", body = crate::error::ErrorBody),
        (status = 409, description = "Conflict", body = crate::error::ErrorBody),
        (status = 500, description = "Internal error", body = crate::error::ErrorBody)
    )
)]
pub async fn deprecate_module<C, E, I>(
    Extension(state): Extension<AppState<C, E>>,
    Extension(identity): Extension<I>,
    headers: HeaderMap,
    Json(body): Json<DeprecateBody>,
) -> Result<impl IntoResponse, ApiError>
where
    C: Catalog + Clone + Send + Sync + 'static,
    E: CatalogHttpErrorMap + Clone + Send + Sync + 'static,
    I: CallerIdentity,
{
    deprecate_inner(&state, &headers, &identity, CatalogKind::Module, body).await
}

#[utoipa::path(
    post,
    path = "/catalog/v1/stack/deprecate",
    tag = "catalog",
    request_body = DeprecateBody,
    responses(
        (status = 204, description = "Stack deprecated"),
        (status = 401, description = "Management authentication required", body = crate::error::ErrorBody),
        (status = 400, description = "Bad request", body = crate::error::ErrorBody),
        (status = 409, description = "Conflict", body = crate::error::ErrorBody),
        (status = 500, description = "Internal error", body = crate::error::ErrorBody)
    )
)]
pub async fn deprecate_stack<C, E, I>(
    Extension(state): Extension<AppState<C, E>>,
    Extension(identity): Extension<I>,
    headers: HeaderMap,
    Json(body): Json<DeprecateBody>,
) -> Result<impl IntoResponse, ApiError>
where
    C: Catalog + Clone + Send + Sync + 'static,
    E: CatalogHttpErrorMap + Clone + Send + Sync + 'static,
    I: CallerIdentity,
{
    deprecate_inner(&state, &headers, &identity, CatalogKind::Stack, body).await
}

async fn deprecate_inner<C, E, I>(
    state: &AppState<C, E>,
    headers: &HeaderMap,
    identity: &I,
    kind: CatalogKind,
    body: DeprecateBody,
) -> Result<impl IntoResponse, ApiError>
where
    C: Catalog,
    E: CatalogHttpErrorMap,
    I: CallerIdentity,
{
    require_management_auth(state, headers, identity)?;
    let reference = ref_from_wire(body.reference);
    state
        .catalog
        .deprecate(kind, &reference, &body.reason)
        .await
        .map_err(|e| state.map_err(e))?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/catalog/v1/provider/yank",
    tag = "catalog",
    request_body = YankBody,
    responses(
        (status = 204, description = "Provider yanked"),
        (status = 401, description = "Management authentication required", body = crate::error::ErrorBody),
        (status = 400, description = "Bad request", body = crate::error::ErrorBody),
        (status = 409, description = "Conflict", body = crate::error::ErrorBody),
        (status = 500, description = "Internal error", body = crate::error::ErrorBody)
    )
)]
pub async fn yank_provider<C, E, I>(
    Extension(state): Extension<AppState<C, E>>,
    Extension(identity): Extension<I>,
    headers: HeaderMap,
    Json(body): Json<YankBody>,
) -> Result<impl IntoResponse, ApiError>
where
    C: Catalog + Clone + Send + Sync + 'static,
    E: CatalogHttpErrorMap + Clone + Send + Sync + 'static,
    I: CallerIdentity,
{
    yank_inner(&state, &headers, &identity, CatalogKind::Provider, body).await
}

#[utoipa::path(
    post,
    path = "/catalog/v1/module/yank",
    tag = "catalog",
    request_body = YankBody,
    responses(
        (status = 204, description = "Module yanked"),
        (status = 401, description = "Management authentication required", body = crate::error::ErrorBody),
        (status = 400, description = "Bad request", body = crate::error::ErrorBody),
        (status = 409, description = "Conflict", body = crate::error::ErrorBody),
        (status = 500, description = "Internal error", body = crate::error::ErrorBody)
    )
)]
pub async fn yank_module<C, E, I>(
    Extension(state): Extension<AppState<C, E>>,
    Extension(identity): Extension<I>,
    headers: HeaderMap,
    Json(body): Json<YankBody>,
) -> Result<impl IntoResponse, ApiError>
where
    C: Catalog + Clone + Send + Sync + 'static,
    E: CatalogHttpErrorMap + Clone + Send + Sync + 'static,
    I: CallerIdentity,
{
    yank_inner(&state, &headers, &identity, CatalogKind::Module, body).await
}

#[utoipa::path(
    post,
    path = "/catalog/v1/stack/yank",
    tag = "catalog",
    request_body = YankBody,
    responses(
        (status = 204, description = "Stack yanked"),
        (status = 401, description = "Management authentication required", body = crate::error::ErrorBody),
        (status = 400, description = "Bad request", body = crate::error::ErrorBody),
        (status = 409, description = "Conflict", body = crate::error::ErrorBody),
        (status = 500, description = "Internal error", body = crate::error::ErrorBody)
    )
)]
pub async fn yank_stack<C, E, I>(
    Extension(state): Extension<AppState<C, E>>,
    Extension(identity): Extension<I>,
    headers: HeaderMap,
    Json(body): Json<YankBody>,
) -> Result<impl IntoResponse, ApiError>
where
    C: Catalog + Clone + Send + Sync + 'static,
    E: CatalogHttpErrorMap + Clone + Send + Sync + 'static,
    I: CallerIdentity,
{
    yank_inner(&state, &headers, &identity, CatalogKind::Stack, body).await
}

async fn yank_inner<C, E, I>(
    state: &AppState<C, E>,
    headers: &HeaderMap,
    identity: &I,
    kind: CatalogKind,
    body: YankBody,
) -> Result<impl IntoResponse, ApiError>
where
    C: Catalog,
    E: CatalogHttpErrorMap,
    I: CallerIdentity,
{
    require_management_auth(state, headers, identity)?;
    let reference = ref_from_wire(body.reference);
    state
        .catalog
        .yank(kind, &reference)
        .await
        .map_err(|e| state.map_err(e))?;
    Ok(StatusCode::NO_CONTENT)
}
