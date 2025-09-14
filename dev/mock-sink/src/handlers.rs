use axum::{Json, extract::State, http::HeaderMap};
use rumqttc::{AsyncClient, QoS};
use serde_json::Value;
use std::sync::Arc;

use crate::types::{TelemetryIn, TelemetryResp};

#[derive(Clone)]
pub struct AppState {
    pub mqtt: AsyncClient,
}

pub async fn health() -> Json<Value> {
    Json(serde_json::json!({"status":"healthy"}))
}

pub async fn telemetry(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<TelemetryIn>,
) -> Result<Json<TelemetryResp>, (axum::http::StatusCode, String)> {
    let request_id = headers
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("-");

    let topic = format!("argus/devices/{}/telemetry", body.device_id);
    let payload = serde_json::to_vec(&body).unwrap_or_default();

    tracing::info!(%request_id, topic = %topic, device_id = %body.device_id, "telemetry received");

    if let Err(e) = state.mqtt.publish(topic.clone(), QoS::AtLeastOnce, false, payload).await {
        tracing::error!(%request_id, error = %e, "mqtt publish failed");
        return Err((axum::http::StatusCode::BAD_GATEWAY, format!("mqtt publish failed: {e}")));
    }

    tracing::info!(%request_id, forwarded_topic = %topic, "telemetry forwarded to mqtt");
    Ok(Json(TelemetryResp { status: "ok", forwarded_topic: topic }))
}

