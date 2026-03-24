//! OpenAPI document ([utoipa](https://docs.rs/utoipa)) for the catalog HTTP surface.
//!
//! Path strings match the Axum mount: `/catalog/health` and `/catalog/v1/...`.

use axum::Json;
use utoipa::OpenApi;

use crate::error::{ErrorBody, ErrorPayload};
use crate::handler;
use crate::management_handlers::{self, CatalogRefWire, DeprecateBody, PromoteBody, YankBody};
use crate::openapi_types::CatalogJsonBody;
use crate::read_handlers::{self, AttachmentPath, DownloadQueryWire, EntryPath, ListQueryWire};

/// OpenAPI 3 document for the catalog HTTP API.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Catalog HTTP API",
        description = "Read, download, and management operations over the catalog trait surface.",
        version = "0.1.0"
    ),
    paths(
        handler::health,
        read_handlers::list_providers,
        read_handlers::list_modules,
        read_handlers::list_stacks,
        read_handlers::list_module_versions,
        read_handlers::list_stack_versions,
        read_handlers::get_provider_entry,
        read_handlers::get_module_entry,
        read_handlers::get_stack_entry,
        read_handlers::download_provider_artifact,
        read_handlers::download_module_artifact,
        read_handlers::download_stack_artifact,
        read_handlers::list_provider_attachments,
        read_handlers::list_module_attachments,
        read_handlers::list_stack_attachments,
        read_handlers::download_provider_attachment,
        read_handlers::download_module_attachment,
        read_handlers::download_stack_attachment,
        management_handlers::promote_provider,
        management_handlers::promote_module,
        management_handlers::promote_stack,
        management_handlers::deprecate_provider,
        management_handlers::deprecate_module,
        management_handlers::deprecate_stack,
        management_handlers::yank_provider,
        management_handlers::yank_module,
        management_handlers::yank_stack,
    ),
    components(schemas(
        ErrorBody,
        ErrorPayload,
        ListQueryWire,
        DownloadQueryWire,
        EntryPath,
        AttachmentPath,
        CatalogJsonBody,
        CatalogRefWire,
        PromoteBody,
        DeprecateBody,
        YankBody,
    )),
    tags((name = "catalog", description = "Catalog HTTP API"))
)]
pub struct ApiDoc;

/// `GET /openapi.json` — OpenAPI document as JSON.
pub async fn serve_openapi_json() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}

/// Serialize the OpenAPI document as JSON (e.g. for tests).
pub fn openapi_json_value() -> serde_json::Value {
    serde_json::to_value(ApiDoc::openapi()).expect("OpenAPI document serializes to JSON")
}
