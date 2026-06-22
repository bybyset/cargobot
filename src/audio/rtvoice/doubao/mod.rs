pub mod config;
pub mod net_client;
pub mod protocol;
pub mod rt_service;
pub mod request_payloads;
use serde_json;
use crate::{audio::MicrophoneError};
use protocol::ProtocolError;

pub use rt_service::RtService;
pub use config::RuntimeConfig;



#[derive(Debug)]
pub enum DoubaoError {
    NetClient(net_client::NetClientError),
    StartSessionTimeout,
    NotStartConnection,
    NotStartSession,
    Microphone(MicrophoneError),
    Audio(String),
    Protocol(ProtocolError),
    InvalidConfig(String),
    Unsupported(String),
    Io(std::io::Error),
    Json(serde_json::Error),
}

impl std::fmt::Display for DoubaoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NetClient(e) => write!(f, "net client error: {e}"),
            Self::StartSessionTimeout => write!(f, "start session timeout"),
            Self::NotStartConnection => write!(f, "not start connection"),
            Self::NotStartSession => write!(f, "not start session"),
            Self::Microphone(e) => write!(f, "microphone error: {e}"),
            Self::Audio(e) => write!(f, "audio error: {e}"),
            Self::Protocol(e) => write!(f, "protocol error: {e}"),
            Self::InvalidConfig(e) => write!(f, "invalid config: {e}"),
            Self::Unsupported(e) => write!(f, "unsupported: {e}"),
            Self::Io(e) => write!(f, "io error: {e}"),
            Self::Json(e) => write!(f, "json error: {e}"),
        }
    }
}

impl std::error::Error for DoubaoError {}

impl From<std::io::Error> for DoubaoError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for DoubaoError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

impl From<protocol::ProtocolError> for DoubaoError {
    fn from(value: protocol::ProtocolError) -> Self {
        Self::Protocol(value)
    }
}
