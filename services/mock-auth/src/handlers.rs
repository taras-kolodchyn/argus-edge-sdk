use crate::types::{
    DeviceLoginReq, DeviceLoginResp, DeviceRegisterReq, DeviceRegisterResp, TokenValidateReq,
};
use axum::http::HeaderMap;
use axum::{Json, http::StatusCode};
use serde_json::json;
use time::OffsetDateTime;
use uuid::Uuid;

pub async fn register(
    headers: HeaderMap,
    Json(req): Json<DeviceRegisterReq>,
) -> Result<Json<DeviceRegisterResp>, (StatusCode, String)> {
    let request_id = headers
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("-");
    let accept_any =
        std::env::var("MOCK_AUTH_ACCEPT_ANY_SECRET").unwrap_or_else(|_| "true".into()) == "true";
    tracing::info!(%request_id, device_id = %req.device_id, "device register request");
    if !accept_any && req.pre_shared_secret.len() < 6 {
        tracing::warn!(%request_id, device_id = %req.device_id, "device register failed: invalid pre_shared_secret");
        return Err((StatusCode::UNAUTHORIZED, "invalid pre_shared_secret".into()));
    }
    let exp = OffsetDateTime::now_utc() + time::Duration::days(7);
    let expires_at = exp
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap();
    let resp = DeviceRegisterResp {
        device_id: req.device_id,
        token: Uuid::new_v4().to_string(),
        mqtt_username: std::env::var("MQTT_USERNAME").unwrap_or_else(|_| "devuser".into()),
        mqtt_password: std::env::var("MQTT_PASSWORD").unwrap_or_else(|_| "devpass".into()),
        expires_at: expires_at.clone(),
    };
    tracing::info!(%request_id, device_id = %resp.device_id, expires_at = %expires_at, "device registered successfully");
    Ok(Json(resp))
}

// --- Login ---

pub async fn login(
    headers: HeaderMap,
    Json(req): Json<DeviceLoginReq>,
) -> Result<Json<DeviceLoginResp>, (StatusCode, String)> {
    let request_id = headers
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("-");
    tracing::info!(%request_id, device_id = %req.device_id, "device login request");
    if req.token.len() < 10 {
        tracing::warn!(%request_id, device_id = %req.device_id, "device login failed: invalid token");
        return Err((StatusCode::UNAUTHORIZED, "invalid token".into()));
    }

    let exp = OffsetDateTime::now_utc() + time::Duration::hours(1);
    let resp = DeviceLoginResp {
        access_token: Uuid::new_v4().to_string(),
        expires_at: exp
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap(),
    };
    tracing::info!(%request_id, device_id = %req.device_id, "device login success");
    Ok(Json(resp))
}

// --- Validate ---

pub async fn validate(
    headers: HeaderMap,
    Json(req): Json<TokenValidateReq>,
) -> Json<serde_json::Value> {
    let request_id = headers
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("-");
    let valid = req.access_token.len() > 10;
    tracing::info!(%request_id, valid = valid, "token validate");
    Json(json!({ "valid": valid }))
}
