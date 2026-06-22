pub mod manager;
pub mod resources;
pub mod softap;

use log::info;
use thiserror::Error;

pub use crate::wifi::manager::WifiManager;

#[derive(Error, Debug)]
pub enum WifiError {
    #[error("ESP-IDF错误: {0}")]
    EspError(#[from] esp_idf_svc::sys::EspError),

    #[error("连接超时")]
    ConnectionTimeout,

    #[error("认证失败")]
    AuthenticationFailed,

    #[error("网络未找到")]
    NetworkNotFound,

    #[error("配置错误: {0}")]
    ConfigError(String),

    #[error("HTTP服务器错误: {0}")]
    HttpServerError(String),
}

pub type Result<T> = std::result::Result<T, WifiError>;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WifiState {
    Unconfigured,
    Configuring,
    Connecting,
    Connected,
    Disconnected,
    Error,
}

// 等待WiFi连接
/// 如果WiFi未配置，会进入配网模式。
/// 如果连接超时，会返回错误。
/// 如果连接成功，会返回WiFi管理器。
pub fn wait_for_wifi_connection() -> Result<WifiManager> {
    // 启动wifi配置，若未配置则进入配网模式
    let mut wifi_manager = WifiManager::new()?;
    wifi_manager.init()?;

    if wifi_manager.is_configured() {
        info!("✅ 已配置WiFi，尝试连接...");
    } else {
        info!("⚠️ 未配置WiFi，将进入配网模式");
        info!("📱 请按以下步骤操作：");
        info!("   1. 用手机连接WiFi: XiaoLiang-Setup");
        info!("   2. 密码: 12345678");
        info!("   3. 浏览器访问: http://192.168.4.1");
        info!("   4. 输入您的家庭WiFi信息");
    }

    wifi_manager.ensure_connected()?;
    Ok(wifi_manager)
}
