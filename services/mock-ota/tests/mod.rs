use super::{ensure_authorized, AuthContext, TokenValidateResponse};
use axum::{routing::post, Json, Router};
use axum::http::{header, HeaderMap, StatusCode};
use reqwest::Client;
use std::net::SocketAddr;
use tokio::{task::JoinHandle, time::Duration};

async fn spawn_validate_server(
    status: StatusCode,
    response: TokenValidateResponse,
) -> (AuthContext, JoinHandle<()>) {
    let router = Router::new().route(
        "/auth/token/validate",
        post(move |Json::<serde_json::Value>(_)| {
            let response = response.clone();
            async move { (status, Json(response)) }
        }),
    );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });

    let client = Client::builder().build().unwrap();
    let auth = AuthContext {
        client,
        validate_url: format!("http://{addr}/auth/token/validate"),
        required_service: "mock-ota".into(),
    };

    // ensure server is ready
    tokio::time::sleep(Duration::from_millis(50)).await;

    (auth, handle)
}

fn bearer(token: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        format!("Bearer {token}").parse().unwrap(),
    );
    headers
}

#[tokio::test]
async fn ensure_authorized_missing_header() {
    let (auth, handle) = spawn_validate_server(
        StatusCode::OK,
        TokenValidateResponse {
            valid: true,
            service: Some("mock-ota".into()),
        },
    )
    .await;
    let headers = HeaderMap::new();
    let result = ensure_authorized(&auth, &headers).await;
    handle.abort();
    assert!(matches!(result, Err((StatusCode::UNAUTHORIZED, _))));
}

#[tokio::test]
async fn ensure_authorized_invalid_token() {
    let (auth, handle) = spawn_validate_server(
        StatusCode::OK,
        TokenValidateResponse {
            valid: false,
            service: None,
        },
    )
    .await;
    let headers = bearer("bad-token");
    let result = ensure_authorized(&auth, &headers).await;
    handle.abort();
    assert!(matches!(result, Err((StatusCode::UNAUTHORIZED, _))));
}

#[tokio::test]
async fn ensure_authorized_wrong_service() {
    let (auth, handle) = spawn_validate_server(
        StatusCode::OK,
        TokenValidateResponse {
            valid: true,
            service: Some("other-service".into()),
        },
    )
    .await;
    let headers = bearer("good-token");
    let result = ensure_authorized(&auth, &headers).await;
    handle.abort();
    assert!(matches!(result, Err((StatusCode::FORBIDDEN, _))));
}

#[tokio::test]
async fn ensure_authorized_success() {
    let (auth, handle) = spawn_validate_server(
        StatusCode::OK,
        TokenValidateResponse {
            valid: true,
            service: Some("mock-ota".into()),
        },
    )
    .await;
    let headers = bearer("good-token");
    let result = ensure_authorized(&auth, &headers).await;
    handle.abort();
    assert!(result.is_ok());
}
