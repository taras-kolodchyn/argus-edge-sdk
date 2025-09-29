use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use rumqttc::{AsyncClient, QoS};
use serde_json::Value;
use std::sync::Arc;

use crate::types::{TelemetryIn, TelemetryResp};

#[derive(Clone)]
pub struct AppState {
    pub mqtt: AsyncClient,
    pub topic_prefix: String,
}

pub async fn health() -> Json<Value> {
    Json(serde_json::json!({"status":"healthy"}))
}

pub async fn telemetry(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<TelemetryIn>,
) -> Result<Json<TelemetryResp>, (StatusCode, String)> {
    let request_id = headers
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("-");

    let device_id = body.device_id.clone();
    let topic = format!("{}{}", state.topic_prefix, device_id);
    let payload = serde_json::to_vec(&body).map_err(|e| {
        tracing::error!(%request_id, error = %e, "serialize telemetry failed");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "serialize telemetry failed".to_string(),
        )
    })?;

    tracing::info!(%request_id, topic = %topic, device_id = %body.device_id, "telemetry received");

    let mqtt = state.mqtt.clone();
    let publish_topic = topic.clone();
    let publish_request_id = request_id.to_string();
    tokio::spawn(async move {
        if let Err(e) = mqtt
            .publish(publish_topic.clone(), QoS::AtLeastOnce, false, payload)
            .await
        {
            tracing::error!(%publish_request_id, topic = %publish_topic, error = %e, "mqtt publish failed");
        } else {
            tracing::info!(%publish_request_id, forwarded_topic = %publish_topic, "telemetry forwarded to mqtt");
        }
    });

    Ok(Json(TelemetryResp {
        status: "ok",
        forwarded_topic: topic,
    }))
}
