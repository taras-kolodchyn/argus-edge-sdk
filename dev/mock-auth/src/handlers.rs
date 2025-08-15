use axum::{Json, http::StatusCode};
use time::OffsetDateTime;
use uuid::Uuid;
use crate::types::{DeviceRegisterReq, DeviceRegisterResp};

pub async fn register(Json(req): Json<DeviceRegisterReq>) -> Result<Json<DeviceRegisterResp>, (StatusCode, String)> {
    let accept_any = std::env::var("MOCK_AUTH_ACCEPT_ANY_SECRET").unwrap_or_else(|_| "true".into()) == "true";
    if !accept_any && req.pre_shared_secret.len() < 6 {
        return Err((StatusCode::UNAUTHORIZED, "invalid pre_shared_secret".into()));
    }
    let exp = OffsetDateTime::now_utc() + time::Duration::days(7);
    let resp = DeviceRegisterResp {
        device_id: req.device_id,
        token: Uuid::new_v4().to_string(),
        mqtt_username: std::env::var("MQTT_USERNAME").unwrap_or_else(|_| "devuser".into()),
        mqtt_password: std::env::var("MQTT_PASSWORD").unwrap_or_else(|_| "devpass".into()),
        expires_at: exp.format(&time::format_description::well_known::Rfc3339).unwrap(),
    };
    Ok(Json(resp))
}
