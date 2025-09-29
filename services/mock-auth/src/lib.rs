use axum::{
    Router,
    routing::{get, post},
};
use serde_json::json;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::trace::TraceLayer;

pub mod handlers;
pub mod types;

pub fn build_router() -> Router {
    Router::new()
        .route("/auth/device/register", post(handlers::register))
        .route("/auth/device/login", post(handlers::login))
        .route("/auth/token/validate", post(handlers::validate))
        .route("/auth/service/login", post(handlers::service_login))
        .route(
            "/healthz",
            get(|| async { axum::Json(json!({"status": "ok"})) }),
        )
        .layer(
            TraceLayer::new_for_http().make_span_with(|req: &axum::http::Request<_>| {
                let request_id = req
                    .headers()
                    .get("x-request-id")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("-");
                tracing::info_span!(
                    "http",
                    %request_id,
                    method = %req.method(),
                    uri = %req.uri(),
                )
            }),
        )
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
}
