//! Host-agnostic caller identity for management routes.

use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;

/// Per-request caller context (authorizer, IAM, etc.). Hosts implement this; API Gateway glue
/// lives in `catalog-aws-apigw`.
pub trait CallerIdentity: Clone + Send + Sync + 'static {
    fn from_request<B>(req: &http::Request<B>) -> Self;
    /// Whether the host attached authorizer context (JWT/IAM/Lambda authorizer payload).
    fn has_authorizer_context(&self) -> bool;
}

/// Identity used in tests and local servers without API Gateway.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct NoopIdentity;

impl CallerIdentity for NoopIdentity {
    fn from_request<B>(_req: &http::Request<B>) -> Self {
        NoopIdentity
    }

    fn has_authorizer_context(&self) -> bool {
        false
    }
}

/// Inserts [`CallerIdentity`] into request extensions before inner handlers run.
pub async fn inject_caller_identity<I: CallerIdentity>(mut req: Request, next: Next) -> Response {
    let id = I::from_request(&req);
    req.extensions_mut().insert(id);
    next.run(req).await
}
