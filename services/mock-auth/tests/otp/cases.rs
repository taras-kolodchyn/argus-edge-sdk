use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use mock_auth::build_router;
use serde_json::{json, Value};
use tower::util::ServiceExt; // for `oneshot`

#[tokio::test]
#[serial_test::serial]
async fn healthz_ok() {
    let app = build_router();
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
#[serial_test::serial]
async fn register_ok() {
    unsafe {
        std::env::set_var("MOCK_AUTH_ACCEPT_ANY_SECRET", "true");
    }
    let app = build_router();
    let body = json!({
        "device_id": "test-device",
        "pre_shared_secret": "secret123",
    })
    .to_string();

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/device/register")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), 64 * 1024).await.unwrap();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["device_id"], "test-device");
    assert!(v["token"].as_str().unwrap().len() > 0);
}

#[tokio::test]
#[serial_test::serial]
async fn register_rejects_short_secret_when_disabled() {
    unsafe {
        std::env::set_var("MOCK_AUTH_ACCEPT_ANY_SECRET", "false");
    }
    let app = build_router();
    let body = json!({
        "device_id": "test-device",
        "pre_shared_secret": "123",
    })
    .to_string();
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/device/register")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[serial_test::serial]
async fn login_then_validate() {
    unsafe {
        std::env::set_var("MOCK_AUTH_ACCEPT_ANY_SECRET", "true");
    }
    let app = build_router();

    // register
    let reg_body = json!({
        "device_id": "test-device",
        "pre_shared_secret": "secret123",
    })
    .to_string();
    let reg_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/device/register")
                .header("content-type", "application/json")
                .body(Body::from(reg_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(reg_resp.status(), StatusCode::OK);
    let reg_json: Value =
        serde_json::from_slice(&to_bytes(reg_resp.into_body(), 64 * 1024).await.unwrap()).unwrap();
    let token = reg_json["token"].as_str().unwrap().to_string();

    // login
    let login_body = json!({
        "device_id": "test-device",
        "token": token,
    })
    .to_string();
    let login_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/device/login")
                .header("content-type", "application/json")
                .body(Body::from(login_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(login_resp.status(), StatusCode::OK);
    let login_json: Value =
        serde_json::from_slice(&to_bytes(login_resp.into_body(), 64 * 1024).await.unwrap())
            .unwrap();
    let access_token = login_json["access_token"].as_str().unwrap();
    assert!(!access_token.is_empty());

    // validate
    let val_body = json!({
        "access_token": access_token,
    })
    .to_string();
    let val_resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/token/validate")
                .header("content-type", "application/json")
                .body(Body::from(val_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(val_resp.status(), StatusCode::OK);
    let val_json: Value =
        serde_json::from_slice(&to_bytes(val_resp.into_body(), 64 * 1024).await.unwrap()).unwrap();
    assert_eq!(val_json["valid"], true);
    assert!(val_json["service"].is_null());
}

#[tokio::test]
#[serial_test::serial]
async fn service_login_and_validate() {
    unsafe {
        std::env::set_var("MOCK_OTA_SERVICE_SECRET", "super-secret");
        std::env::set_var("MOCK_OTA_SERVICE_NAME", "mock-ota");
    }
    let app = build_router();

    let login_body = json!({
        "service": "mock-ota",
        "secret": "super-secret",
    })
    .to_string();
    let login_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/service/login")
                .header("content-type", "application/json")
                .body(Body::from(login_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(login_resp.status(), StatusCode::OK);
    let login_json: Value =
        serde_json::from_slice(&to_bytes(login_resp.into_body(), 64 * 1024).await.unwrap())
            .unwrap();
    let token = login_json["access_token"].as_str().unwrap();

    let validate_body = json!({
        "access_token": token,
    })
    .to_string();
    let validate_resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/token/validate")
                .header("content-type", "application/json")
                .body(Body::from(validate_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(validate_resp.status(), StatusCode::OK);
    let resp_json: Value = serde_json::from_slice(
        &to_bytes(validate_resp.into_body(), 64 * 1024)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(resp_json["valid"], true);
    assert_eq!(resp_json["service"], Value::String("mock-ota".into()));
}
