use serde::{Deserialize, Serialize};

// Incoming payload for HTTP POST /telemetry
#[derive(Debug, Deserialize, Serialize)]
pub struct TelemetryIn {
    pub device_id: String,
    pub temp: Option<f32>,
    pub pm25: Option<f32>,
    pub noise: Option<f32>,
    pub ts: Option<u64>,
}

// Response body for POST /telemetry
#[derive(Debug, Serialize)]
pub struct TelemetryResp {
    pub status: &'static str,
    pub forwarded_topic: String,
}
