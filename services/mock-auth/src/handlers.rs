use crate::types::{
    DeviceLoginReq, DeviceLoginResp, DeviceRegisterReq, DeviceRegisterResp, ServiceLoginReq,
    ServiceLoginResp, TokenValidateReq, TokenValidateResp,
};
use axum::http::HeaderMap;
use axum::{Json, http::StatusCode};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use time::OffsetDateTime;
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Clone)]
struct ServiceTokenInfo {
    service: String,
    expires_at: OffsetDateTime,
}

static SERVICE_TOKENS: Lazy<RwLock<HashMap<String, ServiceTokenInfo>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

fn cleanup_expired(tokens: &mut HashMap<String, ServiceTokenInfo>) {
    let now = OffsetDateTime::now_utc();
    tokens.retain(|_, info| info.expires_at > now);
}

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

// --- Service login ---

pub async fn service_login(
    headers: HeaderMap,
    Json(req): Json<ServiceLoginReq>,
) -> Result<Json<ServiceLoginResp>, (StatusCode, String)> {
    let request_id = headers
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("-");
    let expected_service =
        std::env::var("MOCK_OTA_SERVICE_NAME").unwrap_or_else(|_| "mock-ota".into());
    let expected_secret =
        std::env::var("MOCK_OTA_SERVICE_SECRET").unwrap_or_else(|_| "ota-dev-secret".into());
    if req.service != expected_service {
        tracing::warn!(%request_id, service = %req.service, "service login failed: invalid service");
        return Err((StatusCode::UNAUTHORIZED, "invalid service".into()));
    }
    if req.secret != expected_secret {
        tracing::warn!(%request_id, service = %req.service, "service login failed: invalid secret");
        return Err((StatusCode::UNAUTHORIZED, "invalid secret".into()));
    }

    let expires_at_dt = OffsetDateTime::now_utc() + time::Duration::hours(1);
    let expires_at = expires_at_dt
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap();
    let token = Uuid::new_v4().to_string();

    {
        let mut store = SERVICE_TOKENS.write().await;
        cleanup_expired(&mut store);
        store.insert(
            token.clone(),
            ServiceTokenInfo {
                service: req.service.clone(),
                expires_at: expires_at_dt,
            },
        );
    }

    tracing::info!(%request_id, service = %req.service, "service login success");
    Ok(Json(ServiceLoginResp {
        access_token: token,
        expires_at,
    }))
}

// --- Validate ---

pub async fn validate(
    headers: HeaderMap,
    Json(req): Json<TokenValidateReq>,
) -> Json<TokenValidateResp> {
    let request_id = headers
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("-");
    let mut service = None;
    let mut valid = false;

    {
        let mut store = SERVICE_TOKENS.write().await;
        cleanup_expired(&mut store);
        if let Some(info) = store.get(req.access_token.as_str()) {
            valid = true;
            service = Some(info.service.clone());
        }
    }

    if !valid {
        valid = req.access_token.len() > 10;
    }

    tracing::info!(%request_id, %valid, service = ?service, "token validate");
    Json(TokenValidateResp { valid, service })
}
