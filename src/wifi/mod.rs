pub mod manager;
pub mod softap;
pub mod resources;

use thiserror::Error;

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
