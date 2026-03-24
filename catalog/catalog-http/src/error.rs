//! JSON error bodies and pluggable mapping from [`anyhow::Error`] to [`ApiError`].

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use utoipa::ToSchema;

/// JSON shape returned for [`ApiError`] responses (OpenAPI + wire contract).
#[derive(Serialize, ToSchema)]
pub struct ErrorBody {
    pub error: ErrorPayload,
}

#[derive(Serialize, ToSchema)]
pub struct ErrorPayload {
    code: &'static str,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<serde_json::Value>,
}

/// Structured API failure mapped to status + JSON (see catalog HTTP contract docs).
#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl ApiError {
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "BAD_REQUEST",
            message: message.into(),
        }
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            code: "UNAUTHORIZED",
            message: message.into(),
        }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            code: "NOT_FOUND",
            message: message.into(),
        }
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            code: "CONFLICT",
            message: message.into(),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "INTERNAL_ERROR",
            message: message.into(),
        }
    }

    pub fn internal_path_unavailable() -> Self {
        Self::internal("artifact content is not available via filesystem path in this deployment")
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = ErrorBody {
            error: ErrorPayload {
                code: self.code,
                message: self.message,
                details: None,
            },
        };
        (self.status, Json(body)).into_response()
    }
}

/// Maps catalog trait errors to stable HTTP responses. Hosts supply AWS-aware or other
/// implementations without `catalog-http` depending on backend crates.
pub trait CatalogHttpErrorMap: Clone + Send + Sync + 'static {
    fn map_anyhow(&self, err: anyhow::Error) -> ApiError;
}

/// Default mapper: logs and returns `INTERNAL_ERROR` (no backend-specific downcasts).
#[derive(Clone, Copy, Debug, Default)]
pub struct DefaultCatalogHttpErrorMap;

impl CatalogHttpErrorMap for DefaultCatalogHttpErrorMap {
    fn map_anyhow(&self, err: anyhow::Error) -> ApiError {
        tracing::error!(error = %err, "unhandled catalog error");
        ApiError::internal("internal error".to_string())
    }
}
