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
