use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use esp_idf_svc::http::server::{Configuration as HttpConfig, EspHttpServer};
use esp_idf_svc::http::Method;
use esp_idf_svc::wifi::{AccessPointConfiguration, AuthMethod, Configuration, EspWifi};
use heapless::String as HeaplessString;

use crate::utils::config::{AppConfig, WifiCredentials};
use crate::wifi::{Result, WifiError};
use crate::wifi::resources::CONFIG_PAGE_HTML;

pub struct SoftApServer {
    config: AppConfig,
    credentials_received: Arc<Mutex<Option<WifiCredentials>>>,
}

impl SoftApServer {
    pub fn new(config: AppConfig) -> Self {
        Self {
            config,
            credentials_received: Arc::new(Mutex::new(None)),
        }
    }

    pub fn start(&self) -> Result<EspHttpServer<'static>> {
        let mut server_config = HttpConfig::default();
        server_config.stack_size = 8192;
        
        let mut server = EspHttpServer::new(&server_config)
            .map_err(|e| WifiError::HttpServerError(format!("{:?}", e)))?;

        let credentials = self.credentials_received.clone();

        server.fn_handler("/", Method::Get, |req| -> std::result::Result<(), WifiError> {
            req.into_ok_response()
                .map_err(|e| WifiError::HttpServerError(format!("{:?}", e)))?
                .write(CONFIG_PAGE_HTML.as_bytes())
                .map_err(|e| WifiError::HttpServerError(format!("{:?}", e)))?;
            Ok(())
        }).map_err(|e| WifiError::HttpServerError(format!("{:?}", e)))?;

        server.fn_handler("/config", Method::Post, move |mut req| -> std::result::Result<(), WifiError> {
            let mut body = vec![0u8; 512];
            let bytes_read = req.read(&mut body).unwrap_or(0);
            body.truncate(bytes_read);
            
            let body_str = String::from_utf8_lossy(&body);
            log::info!("收到配网请求: {}", body_str);
            
            if let Ok(creds) = Self::parse_form_data(&body_str) {
                let mut guard = credentials.lock().unwrap();
                *guard = Some(creds);
                req.into_ok_response()
                    .map_err(|e| WifiError::HttpServerError(format!("{:?}", e)))?
                    .write(r#"{"status":"success","message":"配置成功，设备即将重启"}"#.as_bytes())
                    .map_err(|e| WifiError::HttpServerError(format!("{:?}", e)))?;
            } else {
                req.into_status_response(400)
                    .map_err(|e| WifiError::HttpServerError(format!("{:?}", e)))?
                    .write(r#"{"status":"error","message":"参数格式错误"}"#.as_bytes())
                    .map_err(|e| WifiError::HttpServerError(format!("{:?}", e)))?;
            }
            Ok(())
        }).map_err(|e| WifiError::HttpServerError(format!("{:?}", e)))?;

        server.fn_handler("/scan", Method::Get, |req| -> std::result::Result<(), WifiError> {
            let mock_networks = r#"[
                {"ssid":"HomeWiFi_5G","rssi":-45,"auth":true},
                {"ssid":"ChinaNet-1234","rssi":-62,"auth":true},
                {"ssid":"CMCC-Free","rssi":-78,"auth":false}
            ]"#;
            req.into_ok_response()
                .map_err(|e| WifiError::HttpServerError(format!("{:?}", e)))?
                .write(mock_networks.as_bytes())
                .map_err(|e| WifiError::HttpServerError(format!("{:?}", e)))?;
            Ok(())
        }).map_err(|e| WifiError::HttpServerError(format!("{:?}", e)))?;

        log::info!("Web服务器已启动，访问 http://192.168.71.1 进行配网");
        Ok(server)
    }

    pub fn wait_for_credentials(&self, timeout_secs: u64) -> Option<WifiCredentials> {
        for _ in 0..timeout_secs {
            thread::sleep(Duration::from_secs(1));
            if let Ok(guard) = self.credentials_received.lock() {
                if let Some(creds) = guard.as_ref() {
                    return Some(creds.clone());
                }
            }
        }
        None
    }

    fn parse_form_data(data: &str) -> std::result::Result<WifiCredentials, ()> {
        let mut ssid = None;
        let mut password = None;

        for pair in data.split('&') {
            let mut kv = pair.splitn(2, '=');
            if let (Some(key), Some(value)) = (kv.next(), kv.next()) {
                let decoded = url_decode(value);
                match key {
                    "ssid" => ssid = Some(decoded),
                    "password" => password = Some(decoded),
                    _ => {}
                }
            }
        }

        if let Some(ssid) = ssid {
            Ok(WifiCredentials {
                ssid,
                password: password.unwrap_or_default(),
            })
        } else {
            Err(())
        }
    }
}

fn url_decode(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars().peekable();
    
    while let Some(c) = chars.next() {
        if c == '%' {
            if let (Some(h1), Some(h2)) = (chars.next(), chars.next()) {
                if let Ok(byte) = u8::from_str_radix(&format!("{}{}", h1, h2), 16) {
                    result.push(byte as char);
                }
            }
        } else if c == '+' {
            result.push(' ');
        } else {
            result.push(c);
        }
    }
    
    result
}

pub fn start_softap_mode(
    wifi: &mut EspWifi<'static>,
    config: &AppConfig,
) -> Result<()> {
    use core::str::FromStr;

    let ssid: HeaplessString<32> = HeaplessString::from_str(&config.ap_ssid)
        .map_err(|_| WifiError::ConfigError("AP SSID过长".to_string()))?;
    let password: HeaplessString<64> = HeaplessString::from_str(&config.ap_password)
        .map_err(|_| WifiError::ConfigError("AP密码过长".to_string()))?;

    let ap_config = AccessPointConfiguration {
        ssid,
        password,
        auth_method: AuthMethod::WPA2Personal,
        max_connections: 4,
        ..Default::default()
    };

    wifi.set_configuration(&Configuration::AccessPoint(ap_config))?;
    
    // 启动WiFi
    wifi.start()?;
    log::info!("WiFi已启动");
    
    // 等待WiFi启动完成
    thread::sleep(Duration::from_secs(2));
    
    log::info!("SoftAP模式已启动");
    log::info!("WiFi名称: {}", config.ap_ssid);
    log::info!("WiFi密码: {}", config.ap_password);
    log::info!("请连接该WiFi后访问 http://192.168.71.1 进行配网");
    
    Ok(())
}
