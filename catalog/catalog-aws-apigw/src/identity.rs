//! API Gateway request identity (authorizer context) and compatibility with [`catalog_http::CallerIdentity`].
//!
//! Management routes (Phase 5) require authentication when
//! `CATALOG_HTTP_REQUIRE_AUTH_FOR_MANAGEMENT`, `CATALOG_AWS_APIGW_REQUIRE_AUTH_FOR_MANAGEMENT`, or
//! the older `CATALOG_APIGW_REQUIRE_AUTH_FOR_MANAGEMENT` is set;
//! see [`catalog_http::management_handlers`].

use axum::extract::Request;
use catalog_http::CallerIdentity;

/// Caller context derived from API Gateway HTTP API (v2) `requestContext.authorizer`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ApiGatewayIdentity {
    /// Serialized `authorizer` object when present (JWT, IAM, or Lambda authorizer payload).
    pub authorizer_json: Option<String>,
}

/// Extract identity from any [`http::Request`] that carries API Gateway context (Lambda proxy).
/// Non–API-Gateway triggers and unit tests without context yield [`Default`].
#[cfg(feature = "aws")]
pub fn identity_from_request<B>(req: &Request<B>) -> ApiGatewayIdentity {
    use lambda_http::{request::RequestContext, RequestExt};
    match req.request_context_ref() {
        Some(RequestContext::ApiGatewayV2(ctx)) => ApiGatewayIdentity {
            authorizer_json: ctx
                .authorizer
                .as_ref()
                .and_then(|a| serde_json::to_string(a).ok()),
        },
        _ => ApiGatewayIdentity::default(),
    }
}

#[cfg(not(feature = "aws"))]
pub fn identity_from_request<B>(_req: &Request<B>) -> ApiGatewayIdentity {
    ApiGatewayIdentity::default()
}

impl CallerIdentity for ApiGatewayIdentity {
    fn from_request<B>(req: &http::Request<B>) -> Self {
        identity_from_request(req)
    }

    fn has_authorizer_context(&self) -> bool {
        self.authorizer_json.is_some()
    }
}
