use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct DeviceRegisterReq {
    pub device_id: String,
    pub pre_shared_secret: String,
}

#[derive(Serialize)]
pub struct DeviceRegisterResp {
    pub device_id: String,
    pub token: String,
    pub mqtt_username: String,
    pub mqtt_password: String,
    pub expires_at: String,
}

#[derive(Deserialize)]
pub struct DeviceLoginReq {
    pub device_id: String,
    pub token: String,
}

#[derive(Deserialize)]
pub struct TokenValidateReq {
    pub access_token: String,
}

#[derive(Serialize)]
pub struct DeviceLoginResp {
    pub access_token: String,
    pub expires_at: String,
}

#[derive(Deserialize)]
pub struct ServiceLoginReq {
    pub service: String,
    pub secret: String,
}

#[derive(Serialize)]
pub struct ServiceLoginResp {
    pub access_token: String,
    pub expires_at: String,
}

#[derive(Serialize)]
pub struct TokenValidateResp {
    pub valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service: Option<String>,
}
