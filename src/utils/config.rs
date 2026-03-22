use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WifiCredentials {
    pub ssid: String,
    pub password: String,
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub device_name: String,
    pub ap_ssid: String,
    pub ap_password: String,
    pub web_server_port: u16,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            device_name: "小梁语音助手".to_string(),
            ap_ssid: "XiaoLiang-Setup".to_string(),
            ap_password: "12345678".to_string(),
            web_server_port: 80,
        }
    }
}
