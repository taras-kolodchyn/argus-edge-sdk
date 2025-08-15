use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Telemetry {
    pub temp: Option<f32>,
    pub pm25: Option<f32>,
    pub noise: Option<f32>,
    pub ts: Option<u64>,
}
