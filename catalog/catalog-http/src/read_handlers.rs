use axum::body::Body;
use axum::extract::{Extension, Path, Query};
use axum::http::header::{CONTENT_TYPE, LOCATION};
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use catalog_trait::read::{ContentSource, Module, Page, Provider, Query as ListQuery, Stack};
use catalog_trait::types::VersionSelector;
use catalog_trait::Catalog;
use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};

use crate::error::{ApiError, CatalogHttpErrorMap};
use crate::state::AppState;

#[derive(Debug, Deserialize, IntoParams, ToSchema)]
#[into_params(parameter_in = Query)]
pub struct ListQueryWire {
    name: Option<String>,
    track: Option<String>,
    limit: Option<String>,
    next: Option<String>,
    projection: Option<String>,
}

#[derive(Debug, Deserialize, Default, IntoParams, ToSchema)]
#[into_params(parameter_in = Query)]
pub struct DownloadQueryWire {
    redirect: Option<String>,
}

#[derive(Debug, Deserialize, IntoParams, ToSchema)]
#[into_params(parameter_in = Path)]
pub struct EntryPath {
    pub track: String,
    pub name: String,
    pub version: String,
}

#[derive(Debug, Deserialize, IntoParams, ToSchema)]
#[into_params(parameter_in = Path)]
pub struct AttachmentPath {
    pub track: String,
    pub name: String,
    pub version: String,
    pub attachment_name: String,
}

fn wants_redirect(q: &DownloadQueryWire) -> bool {
    q.redirect
        .as_ref()
        .is_some_and(|s| matches!(s.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
}

fn wire_to_list_query(w: ListQueryWire) -> Result<ListQuery, ApiError> {
    let limit = match w.limit {
        None => None,
        Some(ref s) if s.trim().is_empty() => None,
        Some(s) => Some(
            s.trim()
                .parse::<u32>()
                .map_err(|_| ApiError::bad_request(format!("invalid limit: {s}")))?,
        ),
    };
    let projection = match w.projection {
        None => None,
        Some(ref s) if s.trim().is_empty() => None,
        Some(s) => Some(parse_projection(&s)?),
    };
    Ok(ListQuery {
        name: w.name.filter(|s| !s.is_empty()),
        track: w.track.filter(|s| !s.is_empty()),
        limit,
        next: w.next.filter(|s| !s.is_empty()),
        projection,
    })
}

fn parse_projection(s: &str) -> Result<catalog_trait::read::ProjectionFields, ApiError> {
    use catalog_trait::read::ProjectionFields;
    let mut acc = ProjectionFields::default();
    for token in s.split(',') {
        let t = token.trim();
        if t.is_empty() {
            continue;
        }
        let mask = match t.to_ascii_lowercase().as_str() {
            "metadata" => ProjectionFields::METADATA,
            "manifest" => ProjectionFields::MANIFEST,
            "terraform" => ProjectionFields::TERRAFORM,
            "stack_data" => ProjectionFields::STACK_DATA,
            "provider_mirror" => ProjectionFields::PROVIDER_MIRROR,
            _ => {
                return Err(ApiError::bad_request(format!(
                    "unknown projection flag: {t}"
                )));
            }
        };
        acc |= mask;
    }
    if acc == ProjectionFields::default() {
        return Err(ApiError::bad_request(
            "projection must include at least one valid flag",
        ));
    }
    Ok(acc)
}

pub fn parse_version_segment(segment: &str) -> Result<VersionSelector, ApiError> {
    if segment.is_empty() {
        return Err(ApiError::bad_request("version segment is empty"));
    }
    if segment.eq_ignore_ascii_case("latest") {
        Ok(VersionSelector::Latest)
    } else {
        Ok(VersionSelector::Exact(segment.to_string()))
    }
}

fn content_source_response(
    source: ContentSource,
    q: &DownloadQueryWire,
) -> Result<Response, ApiError> {
    match source {
        ContentSource::Url(url) => {
            if wants_redirect(q) {
                let hv = HeaderValue::try_from(url.as_str()).map_err(|_| {
                    ApiError::internal("artifact URL is not a valid response header value")
                })?;
                Ok((StatusCode::FOUND, [(LOCATION, hv)]).into_response())
            } else {
                Ok(Json(serde_json::json!({ "url": url })).into_response())
            }
        }
        ContentSource::Bytes(bytes) => Ok((
            StatusCode::OK,
            [(
                CONTENT_TYPE,
                HeaderValue::from_static("application/octet-stream"),
            )],
            Body::from(bytes),
        )
            .into_response()),
        ContentSource::Path(_) => Err(ApiError::internal_path_unavailable()),
    }
}

#[utoipa::path(
    get,
    path = "/catalog/v1/providers",
    tag = "catalog",
    params(ListQueryWire),
    responses(
        (status = 200, description = "Paginated providers", body = crate::openapi_types::CatalogJsonBody),
        (status = 400, description = "Invalid query", body = crate::error::ErrorBody),
        (status = 500, description = "Internal error", body = crate::error::ErrorBody)
    )
)]
pub async fn list_providers<C, E>(
    Extension(state): Extension<AppState<C, E>>,
    Query(wire): Query<ListQueryWire>,
) -> Result<Json<Page<Provider>>, ApiError>
where
    C: Catalog + Clone + Send + Sync + 'static,
    E: CatalogHttpErrorMap + Clone + Send + Sync + 'static,
{
    let query = wire_to_list_query(wire)?;
    let page = state
        .catalog
        .list_providers(&query)
        .await
        .map_err(|e| state.map_err(e))?;
    Ok(Json(page))
}

#[utoipa::path(
    get,
    path = "/catalog/v1/modules",
    tag = "catalog",
    params(ListQueryWire),
    responses(
        (status = 200, description = "Paginated modules", body = crate::openapi_types::CatalogJsonBody),
        (status = 400, description = "Invalid query", body = crate::error::ErrorBody),
        (status = 500, description = "Internal error", body = crate::error::ErrorBody)
    )
)]
pub async fn list_modules<C, E>(
    Extension(state): Extension<AppState<C, E>>,
    Query(wire): Query<ListQueryWire>,
) -> Result<Json<Page<Module>>, ApiError>
where
    C: Catalog + Clone + Send + Sync + 'static,
    E: CatalogHttpErrorMap + Clone + Send + Sync + 'static,
{
    let query = wire_to_list_query(wire)?;
    let page = state
        .catalog
        .list_modules(&query)
        .await
        .map_err(|e| state.map_err(e))?;
    Ok(Json(page))
}

#[utoipa::path(
    get,
    path = "/catalog/v1/stacks",
    tag = "catalog",
    params(ListQueryWire),
    responses(
        (status = 200, description = "Paginated stacks", body = crate::openapi_types::CatalogJsonBody),
        (status = 400, description = "Invalid query", body = crate::error::ErrorBody),
        (status = 500, description = "Internal error", body = crate::error::ErrorBody)
    )
)]
pub async fn list_stacks<C, E>(
    Extension(state): Extension<AppState<C, E>>,
    Query(wire): Query<ListQueryWire>,
) -> Result<Json<Page<Stack>>, ApiError>
where
    C: Catalog + Clone + Send + Sync + 'static,
    E: CatalogHttpErrorMap + Clone + Send + Sync + 'static,
{
    let query = wire_to_list_query(wire)?;
    let page = state
        .catalog
        .list_stacks(&query)
        .await
        .map_err(|e| state.map_err(e))?;
    Ok(Json(page))
}

#[utoipa::path(
    get,
    path = "/catalog/v1/modules/versions/{track}/{name}",
    tag = "catalog",
    params(
        ("track" = String, Path, description = "Track"),
        ("name" = String, Path, description = "Module name"),
        ListQueryWire
    ),
    responses(
        (status = 200, description = "Paginated module versions", body = crate::openapi_types::CatalogJsonBody),
        (status = 400, description = "Invalid query", body = crate::error::ErrorBody),
        (status = 500, description = "Internal error", body = crate::error::ErrorBody)
    )
)]
pub async fn list_module_versions<C, E>(
    Extension(state): Extension<AppState<C, E>>,
    Path((track, name)): Path<(String, String)>,
    Query(wire): Query<ListQueryWire>,
) -> Result<Json<Page<Module>>, ApiError>
where
    C: Catalog + Clone + Send + Sync + 'static,
    E: CatalogHttpErrorMap + Clone + Send + Sync + 'static,
{
    let mut query = wire_to_list_query(wire)?;
    query.track = Some(track);
    query.name = Some(name);
    let page = state
        .catalog
        .list_modules(&query)
        .await
        .map_err(|e| state.map_err(e))?;
    Ok(Json(page))
}

#[utoipa::path(
    get,
    path = "/catalog/v1/stacks/versions/{track}/{name}",
    tag = "catalog",
    params(
        ("track" = String, Path, description = "Track"),
        ("name" = String, Path, description = "Stack name"),
        ListQueryWire
    ),
    responses(
        (status = 200, description = "Paginated stack versions", body = crate::openapi_types::CatalogJsonBody),
        (status = 400, description = "Invalid query", body = crate::error::ErrorBody),
        (status = 500, description = "Internal error", body = crate::error::ErrorBody)
    )
)]
pub async fn list_stack_versions<C, E>(
    Extension(state): Extension<AppState<C, E>>,
    Path((track, name)): Path<(String, String)>,
    Query(wire): Query<ListQueryWire>,
) -> Result<Json<Page<Stack>>, ApiError>
where
    C: Catalog + Clone + Send + Sync + 'static,
    E: CatalogHttpErrorMap + Clone + Send + Sync + 'static,
{
    let mut query = wire_to_list_query(wire)?;
    query.track = Some(track);
    query.name = Some(name);
    let page = state
        .catalog
        .list_stacks(&query)
        .await
        .map_err(|e| state.map_err(e))?;
    Ok(Json(page))
}

#[utoipa::path(
    get,
    path = "/catalog/v1/provider/{track}/{name}/{version}",
    tag = "catalog",
    params(EntryPath),
    responses(
        (status = 200, description = "Provider entry", body = crate::openapi_types::CatalogJsonBody),
        (status = 400, description = "Invalid version selector", body = crate::error::ErrorBody),
        (status = 404, description = "Not found", body = crate::error::ErrorBody),
        (status = 500, description = "Internal error", body = crate::error::ErrorBody)
    )
)]
pub async fn get_provider_entry<C, E>(
    Extension(state): Extension<AppState<C, E>>,
    Path(path): Path<EntryPath>,
) -> Result<Json<Provider>, ApiError>
where
    C: Catalog + Clone + Send + Sync + 'static,
    E: CatalogHttpErrorMap + Clone + Send + Sync + 'static,
{
    let version = parse_version_segment(&path.version)?;
    let entry = state
        .catalog
        .get_provider(&path.name, &path.track, version)
        .await
        .map_err(|e| state.map_err(e))?
        .ok_or_else(|| ApiError::not_found("provider not found"))?;
    Ok(Json(entry))
}

#[utoipa::path(
    get,
    path = "/catalog/v1/module/{track}/{name}/{version}",
    tag = "catalog",
    params(EntryPath),
    responses(
        (status = 200, description = "Module entry", body = crate::openapi_types::CatalogJsonBody),
        (status = 400, description = "Invalid version selector", body = crate::error::ErrorBody),
        (status = 404, description = "Not found", body = crate::error::ErrorBody),
        (status = 500, description = "Internal error", body = crate::error::ErrorBody)
    )
)]
pub async fn get_module_entry<C, E>(
    Extension(state): Extension<AppState<C, E>>,
    Path(path): Path<EntryPath>,
) -> Result<Json<Module>, ApiError>
where
    C: Catalog + Clone + Send + Sync + 'static,
    E: CatalogHttpErrorMap + Clone + Send + Sync + 'static,
{
    let version = parse_version_segment(&path.version)?;
    let entry = state
        .catalog
        .get_module(&path.name, &path.track, version)
        .await
        .map_err(|e| state.map_err(e))?
        .ok_or_else(|| ApiError::not_found("module not found"))?;
    Ok(Json(entry))
}

#[utoipa::path(
    get,
    path = "/catalog/v1/stack/{track}/{name}/{version}",
    tag = "catalog",
    params(EntryPath),
    responses(
        (status = 200, description = "Stack entry", body = crate::openapi_types::CatalogJsonBody),
        (status = 400, description = "Invalid version selector", body = crate::error::ErrorBody),
        (status = 404, description = "Not found", body = crate::error::ErrorBody),
        (status = 500, description = "Internal error", body = crate::error::ErrorBody)
    )
)]
pub async fn get_stack_entry<C, E>(
    Extension(state): Extension<AppState<C, E>>,
    Path(path): Path<EntryPath>,
) -> Result<Json<Stack>, ApiError>
where
    C: Catalog + Clone + Send + Sync + 'static,
    E: CatalogHttpErrorMap + Clone + Send + Sync + 'static,
{
    let version = parse_version_segment(&path.version)?;
    let entry = state
        .catalog
        .get_stack(&path.name, &path.track, version)
        .await
        .map_err(|e| state.map_err(e))?
        .ok_or_else(|| ApiError::not_found("stack not found"))?;
    Ok(Json(entry))
}

#[utoipa::path(
    get,
    path = "/catalog/v1/provider/{track}/{name}/{version}/download",
    tag = "catalog",
    params(EntryPath, DownloadQueryWire),
    responses(
        (status = 200, description = "Artifact bytes (application/octet-stream) or JSON url payload", body = crate::openapi_types::CatalogJsonBody),
        (status = 302, description = "Redirect to artifact URL when redirect is set"),
        (status = 400, description = "Invalid version or query", body = crate::error::ErrorBody),
        (status = 404, description = "Not found", body = crate::error::ErrorBody),
        (status = 500, description = "Internal error", body = crate::error::ErrorBody)
    )
)]
pub async fn download_provider_artifact<C, E>(
    Extension(state): Extension<AppState<C, E>>,
    Path(path): Path<EntryPath>,
    Query(q): Query<DownloadQueryWire>,
) -> Result<Response, ApiError>
where
    C: Catalog + Clone + Send + Sync + 'static,
    E: CatalogHttpErrorMap + Clone + Send + Sync + 'static,
{
    let version = parse_version_segment(&path.version)?;
    let entry = state
        .catalog
        .get_provider(&path.name, &path.track, version)
        .await
        .map_err(|e| state.map_err(e))?
        .ok_or_else(|| ApiError::not_found("provider not found"))?;
    let source = state
        .catalog
        .download_provider(&entry.reference)
        .await
        .map_err(|e| state.map_err(e))?;
    content_source_response(source, &q)
}

#[utoipa::path(
    get,
    path = "/catalog/v1/module/{track}/{name}/{version}/download",
    tag = "catalog",
    params(EntryPath, DownloadQueryWire),
    responses(
        (status = 200, description = "Artifact bytes (application/octet-stream) or JSON url payload", body = crate::openapi_types::CatalogJsonBody),
        (status = 302, description = "Redirect to artifact URL when redirect is set"),
        (status = 400, description = "Invalid version or query", body = crate::error::ErrorBody),
        (status = 404, description = "Not found", body = crate::error::ErrorBody),
        (status = 500, description = "Internal error", body = crate::error::ErrorBody)
    )
)]
pub async fn download_module_artifact<C, E>(
    Extension(state): Extension<AppState<C, E>>,
    Path(path): Path<EntryPath>,
    Query(q): Query<DownloadQueryWire>,
) -> Result<Response, ApiError>
where
    C: Catalog + Clone + Send + Sync + 'static,
    E: CatalogHttpErrorMap + Clone + Send + Sync + 'static,
{
    let version = parse_version_segment(&path.version)?;
    let entry = state
        .catalog
        .get_module(&path.name, &path.track, version)
        .await
        .map_err(|e| state.map_err(e))?
        .ok_or_else(|| ApiError::not_found("module not found"))?;
    let source = state
        .catalog
        .download_module(&entry.reference)
        .await
        .map_err(|e| state.map_err(e))?;
    content_source_response(source, &q)
}

#[utoipa::path(
    get,
    path = "/catalog/v1/stack/{track}/{name}/{version}/download",
    tag = "catalog",
    params(EntryPath, DownloadQueryWire),
    responses(
        (status = 200, description = "Artifact bytes (application/octet-stream) or JSON url payload", body = crate::openapi_types::CatalogJsonBody),
        (status = 302, description = "Redirect to artifact URL when redirect is set"),
        (status = 400, description = "Invalid version or query", body = crate::error::ErrorBody),
        (status = 404, description = "Not found", body = crate::error::ErrorBody),
        (status = 500, description = "Internal error", body = crate::error::ErrorBody)
    )
)]
pub async fn download_stack_artifact<C, E>(
    Extension(state): Extension<AppState<C, E>>,
    Path(path): Path<EntryPath>,
    Query(q): Query<DownloadQueryWire>,
) -> Result<Response, ApiError>
where
    C: Catalog + Clone + Send + Sync + 'static,
    E: CatalogHttpErrorMap + Clone + Send + Sync + 'static,
{
    let version = parse_version_segment(&path.version)?;
    let entry = state
        .catalog
        .get_stack(&path.name, &path.track, version)
        .await
        .map_err(|e| state.map_err(e))?
        .ok_or_else(|| ApiError::not_found("stack not found"))?;
    let source = state
        .catalog
        .download_stack(&entry.reference)
        .await
        .map_err(|e| state.map_err(e))?;
    content_source_response(source, &q)
}

#[utoipa::path(
    get,
    path = "/catalog/v1/provider/{track}/{name}/{version}/attachments",
    tag = "catalog",
    params(EntryPath),
    responses(
        (status = 200, description = "Attachment file names", body = Vec<String>),
        (status = 400, description = "Invalid version selector", body = crate::error::ErrorBody),
        (status = 404, description = "Not found", body = crate::error::ErrorBody),
        (status = 500, description = "Internal error", body = crate::error::ErrorBody)
    )
)]
pub async fn list_provider_attachments<C, E>(
    Extension(state): Extension<AppState<C, E>>,
    Path(path): Path<EntryPath>,
) -> Result<Json<Vec<String>>, ApiError>
where
    C: Catalog + Clone + Send + Sync + 'static,
    E: CatalogHttpErrorMap + Clone + Send + Sync + 'static,
{
    let version = parse_version_segment(&path.version)?;
    let entry = state
        .catalog
        .get_provider(&path.name, &path.track, version)
        .await
        .map_err(|e| state.map_err(e))?
        .ok_or_else(|| ApiError::not_found("provider not found"))?;
    let names = state
        .catalog
        .list_attachments(&entry.reference)
        .await
        .map_err(|e| state.map_err(e))?;
    Ok(Json(names))
}

#[utoipa::path(
    get,
    path = "/catalog/v1/module/{track}/{name}/{version}/attachments",
    tag = "catalog",
    params(EntryPath),
    responses(
        (status = 200, description = "Attachment file names", body = Vec<String>),
        (status = 400, description = "Invalid version selector", body = crate::error::ErrorBody),
        (status = 404, description = "Not found", body = crate::error::ErrorBody),
        (status = 500, description = "Internal error", body = crate::error::ErrorBody)
    )
)]
pub async fn list_module_attachments<C, E>(
    Extension(state): Extension<AppState<C, E>>,
    Path(path): Path<EntryPath>,
) -> Result<Json<Vec<String>>, ApiError>
where
    C: Catalog + Clone + Send + Sync + 'static,
    E: CatalogHttpErrorMap + Clone + Send + Sync + 'static,
{
    let version = parse_version_segment(&path.version)?;
    let entry = state
        .catalog
        .get_module(&path.name, &path.track, version)
        .await
        .map_err(|e| state.map_err(e))?
        .ok_or_else(|| ApiError::not_found("module not found"))?;
    let names = state
        .catalog
        .list_attachments(&entry.reference)
        .await
        .map_err(|e| state.map_err(e))?;
    Ok(Json(names))
}

#[utoipa::path(
    get,
    path = "/catalog/v1/stack/{track}/{name}/{version}/attachments",
    tag = "catalog",
    params(EntryPath),
    responses(
        (status = 200, description = "Attachment file names", body = Vec<String>),
        (status = 400, description = "Invalid version selector", body = crate::error::ErrorBody),
        (status = 404, description = "Not found", body = crate::error::ErrorBody),
        (status = 500, description = "Internal error", body = crate::error::ErrorBody)
    )
)]
pub async fn list_stack_attachments<C, E>(
    Extension(state): Extension<AppState<C, E>>,
    Path(path): Path<EntryPath>,
) -> Result<Json<Vec<String>>, ApiError>
where
    C: Catalog + Clone + Send + Sync + 'static,
    E: CatalogHttpErrorMap + Clone + Send + Sync + 'static,
{
    let version = parse_version_segment(&path.version)?;
    let entry = state
        .catalog
        .get_stack(&path.name, &path.track, version)
        .await
        .map_err(|e| state.map_err(e))?
        .ok_or_else(|| ApiError::not_found("stack not found"))?;
    let names = state
        .catalog
        .list_attachments(&entry.reference)
        .await
        .map_err(|e| state.map_err(e))?;
    Ok(Json(names))
}

#[utoipa::path(
    get,
    path = "/catalog/v1/provider/{track}/{name}/{version}/attachments/{attachment_name}",
    tag = "catalog",
    params(AttachmentPath, DownloadQueryWire),
    responses(
        (status = 200, description = "Attachment bytes or JSON url payload", body = crate::openapi_types::CatalogJsonBody),
        (status = 302, description = "Redirect when redirect is set"),
        (status = 400, description = "Invalid version or query", body = crate::error::ErrorBody),
        (status = 404, description = "Not found", body = crate::error::ErrorBody),
        (status = 500, description = "Internal error", body = crate::error::ErrorBody)
    )
)]
pub async fn download_provider_attachment<C, E>(
    Extension(state): Extension<AppState<C, E>>,
    Path(path): Path<AttachmentPath>,
    Query(q): Query<DownloadQueryWire>,
) -> Result<Response, ApiError>
where
    C: Catalog + Clone + Send + Sync + 'static,
    E: CatalogHttpErrorMap + Clone + Send + Sync + 'static,
{
    let version = parse_version_segment(&path.version)?;
    let entry = state
        .catalog
        .get_provider(&path.name, &path.track, version)
        .await
        .map_err(|e| state.map_err(e))?
        .ok_or_else(|| ApiError::not_found("provider not found"))?;
    let source = state
        .catalog
        .download_attachment(&entry.reference, &path.attachment_name)
        .await
        .map_err(|e| state.map_err(e))?;
    content_source_response(source, &q)
}

#[utoipa::path(
    get,
    path = "/catalog/v1/module/{track}/{name}/{version}/attachments/{attachment_name}",
    tag = "catalog",
    params(AttachmentPath, DownloadQueryWire),
    responses(
        (status = 200, description = "Attachment bytes or JSON url payload", body = crate::openapi_types::CatalogJsonBody),
        (status = 302, description = "Redirect when redirect is set"),
        (status = 400, description = "Invalid version or query", body = crate::error::ErrorBody),
        (status = 404, description = "Not found", body = crate::error::ErrorBody),
        (status = 500, description = "Internal error", body = crate::error::ErrorBody)
    )
)]
pub async fn download_module_attachment<C, E>(
    Extension(state): Extension<AppState<C, E>>,
    Path(path): Path<AttachmentPath>,
    Query(q): Query<DownloadQueryWire>,
) -> Result<Response, ApiError>
where
    C: Catalog + Clone + Send + Sync + 'static,
    E: CatalogHttpErrorMap + Clone + Send + Sync + 'static,
{
    let version = parse_version_segment(&path.version)?;
    let entry = state
        .catalog
        .get_module(&path.name, &path.track, version)
        .await
        .map_err(|e| state.map_err(e))?
        .ok_or_else(|| ApiError::not_found("module not found"))?;
    let source = state
        .catalog
        .download_attachment(&entry.reference, &path.attachment_name)
        .await
        .map_err(|e| state.map_err(e))?;
    content_source_response(source, &q)
}

#[utoipa::path(
    get,
    path = "/catalog/v1/stack/{track}/{name}/{version}/attachments/{attachment_name}",
    tag = "catalog",
    params(AttachmentPath, DownloadQueryWire),
    responses(
        (status = 200, description = "Attachment bytes or JSON url payload", body = crate::openapi_types::CatalogJsonBody),
        (status = 302, description = "Redirect when redirect is set"),
        (status = 400, description = "Invalid version or query", body = crate::error::ErrorBody),
        (status = 404, description = "Not found", body = crate::error::ErrorBody),
        (status = 500, description = "Internal error", body = crate::error::ErrorBody)
    )
)]
pub async fn download_stack_attachment<C, E>(
    Extension(state): Extension<AppState<C, E>>,
    Path(path): Path<AttachmentPath>,
    Query(q): Query<DownloadQueryWire>,
) -> Result<Response, ApiError>
where
    C: Catalog + Clone + Send + Sync + 'static,
    E: CatalogHttpErrorMap + Clone + Send + Sync + 'static,
{
    let version = parse_version_segment(&path.version)?;
    let entry = state
        .catalog
        .get_stack(&path.name, &path.track, version)
        .await
        .map_err(|e| state.map_err(e))?
        .ok_or_else(|| ApiError::not_found("stack not found"))?;
    let source = state
        .catalog
        .download_attachment(&entry.reference, &path.attachment_name)
        .await
        .map_err(|e| state.map_err(e))?;
    content_source_response(source, &q)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::NoopIdentity;
    use crate::test_catalog::PresetCatalog;
    use crate::{build_router, AppState};
    use axum::body::to_bytes;
    use axum::body::Body;
    use catalog_trait::read::CatalogEntry;
    use catalog_trait::read::Module;
    use catalog_trait::types::CatalogRef;
    use tower::ServiceExt;

    fn test_app(catalog: PresetCatalog) -> axum::Router {
        build_router::<_, _, NoopIdentity>(AppState::new(catalog))
    }

    #[tokio::test]
    async fn list_modules_returns_page_json() {
        let page = Page {
            items: vec![CatalogEntry::Module(Module::new(CatalogRef {
                id: "ref1".into(),
            }))],
            next: None,
        };
        let catalog = PresetCatalog::with_list_page(page);
        let app = test_app(catalog);
        let res = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/catalog/v1/modules")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(v["items"].is_array());
        assert_eq!(v["items"][0]["reference"]["id"], "ref1");
    }

    #[tokio::test]
    async fn bad_projection_returns_400() {
        let app = test_app(PresetCatalog::default());
        let res = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/catalog/v1/modules?projection=not_a_flag")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn get_module_404_when_missing() {
        let app = test_app(PresetCatalog::default());
        let res = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/catalog/v1/module/main/foo/1.0.0")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn download_module_json_url() {
        let m = Module::new(CatalogRef { id: "key".into() });
        let catalog = PresetCatalog::new()
            .with_get(Ok(Some(CatalogEntry::Module(m))))
            .with_download_module(Ok(ContentSource::Url("https://example.com/bin.zip".into())));
        let app = test_app(catalog);
        let res = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/catalog/v1/module/main/foo/1.0.0/download")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["url"].as_str(), Some("https://example.com/bin.zip"));
    }

    #[tokio::test]
    async fn download_module_redirect() {
        let m = Module::new(CatalogRef { id: "key".into() });
        let catalog = PresetCatalog::new()
            .with_get(Ok(Some(CatalogEntry::Module(m))))
            .with_download_module(Ok(ContentSource::Url("https://example.com/bin.zip".into())));
        let app = test_app(catalog);
        let res = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/catalog/v1/module/main/foo/1.0.0/download?redirect=1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::FOUND);
        let loc = res
            .headers()
            .get(axum::http::header::LOCATION)
            .unwrap()
            .to_str()
            .unwrap();
        assert_eq!(loc, "https://example.com/bin.zip");
    }

    #[tokio::test]
    async fn list_attachments_returns_array() {
        let m = Module::new(CatalogRef { id: "key".into() });
        let catalog = PresetCatalog::new()
            .with_get(Ok(Some(CatalogEntry::Module(m))))
            .with_list_attachments(Ok(vec!["a.json".into(), "b.json".into()]));
        let app = test_app(catalog);
        let res = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/catalog/v1/module/main/foo/1.0.0/attachments")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let names: Vec<String> = serde_json::from_slice(&body).unwrap();
        assert_eq!(names, vec!["a.json", "b.json"]);
    }
}
