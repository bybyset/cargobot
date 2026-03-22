use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::peripherals::Peripherals;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::wifi::{ClientConfiguration, Configuration, EspWifi};

use crate::utils::config::{AppConfig, WifiCredentials};
use crate::utils::nvs::NvsStorage;
use crate::wifi::softap::{start_softap_mode, SoftApServer};
use crate::wifi::{Result, WifiError, WifiState};

use heapless::String as HeaplessString;
use std::str::FromStr;

use std::thread;
use std::time::Duration;

pub struct WifiManager {
    state: WifiState,
    config: AppConfig,
    nvs: NvsStorage,
}

impl WifiManager {
    pub fn new() -> Result<Self> {
        let nvs = NvsStorage::new()?;
        
        Ok(Self {
            state: WifiState::Unconfigured,
            config: AppConfig::default(),
            nvs,
        })
    }

    pub fn init(&mut self) -> Result<()> {
        if self.nvs.has_wifi_credentials() {
            self.state = WifiState::Disconnected;
            log::info!("检测到已保存的WiFi凭证");
        } else {
            self.state = WifiState::Unconfigured;
            log::info!("未检测到WiFi凭证，需要配网");
        }
        Ok(())
    }

    pub fn ensure_connected(&mut self) -> Result<()> {
        match self.state {
            WifiState::Connected => {
                log::info!("WiFi已连接");
                Ok(())
            }
            WifiState::Disconnected => {
                if let Some(creds) = self.nvs.load_wifi_credentials()? {
                    self.connect_to_wifi(creds)
                } else {
                    self.start_provisioning()
                }
            }
            WifiState::Unconfigured => self.start_provisioning(),
            _ => {
                log::warn!("WiFi状态异常: {:?}", self.state);
                self.start_provisioning()
            }
        }
    }

    fn connect_to_wifi(&mut self, creds: WifiCredentials) -> Result<()> {
        self.state = WifiState::Connecting;
        log::info!("正在连接WiFi: {}", creds.ssid);

        let peripherals = Peripherals::take().unwrap();
        let sys_loop = EspSystemEventLoop::take()?;
        let nvs = EspDefaultNvsPartition::take()?;

        let mut wifi = EspWifi::new(
            peripherals.modem,
            sys_loop.clone(),
            Some(nvs),
        )?;

        let ssid = HeaplessString::from_str(&creds.ssid).unwrap();
        let password = HeaplessString::from_str(&creds.password).unwrap();

        let client_config = ClientConfiguration {
            ssid,
            password,
            ..Default::default()
        };

        wifi.set_configuration(&Configuration::Client(client_config))?;
        wifi.start()?;
        wifi.connect()?;

        log::info!("等待WiFi连接...");
        
        let timeout = 30;
        for i in 0..timeout {
            if wifi.is_connected()? {
                if let Ok(ip_info) = wifi.sta_netif().get_ip_info() {
                    log::info!("✅ WiFi连接成功!");
                    log::info!("   SSID: {}", creds.ssid);
                    log::info!("   IP地址: {}", ip_info.ip);
                    self.state = WifiState::Connected;
                    return Ok(());
                }
            }
            thread::sleep(Duration::from_secs(1));
            log::debug!("连接中... {}/{} 秒", i + 1, timeout);
        }

        log::error!("❌ WiFi连接超时");
        self.state = WifiState::Error;
        Err(WifiError::ConnectionTimeout)
    }

    fn start_provisioning(&mut self) -> Result<()> {
        self.state = WifiState::Configuring;
        log::info!("启动配网模式...");

        let peripherals = Peripherals::take().unwrap();
        let sys_loop = EspSystemEventLoop::take()?;
        let nvs = EspDefaultNvsPartition::take()?;

        let mut wifi = EspWifi::new(
            peripherals.modem,
            sys_loop.clone(),
            Some(nvs),
        )?;

        start_softap_mode(&mut wifi, &self.config)?;

        let server = SoftApServer::new(self.config.clone());
        let _http_server = server.start()?;

        log::info!("配网服务器已启动，等待用户配置...");
        log::info!("请在手机上连接WiFi: {}", self.config.ap_ssid);
        log::info!("密码: {}", self.config.ap_password);
        log::info!("然后访问 http://192.168.4.1 进行配网");

        if let Some(creds) = server.wait_for_credentials(300) {
            log::info!("收到WiFi凭证: SSID={}", creds.ssid);
            
            self.nvs.save_wifi_credentials(&creds)?;
            log::info!("凭证已保存，正在重启...");
            
            thread::sleep(Duration::from_secs(2));
            unsafe {
                esp_idf_svc::sys::esp_restart();
            }
            // 不会执行到这里，esp_restart 不会返回
        } else {
            log::warn!("配网超时，未收到配置信息");
            self.state = WifiState::Error;
            Err(WifiError::ConnectionTimeout)
        }
    }

    pub fn clear_credentials(&mut self) -> Result<()> {
        self.nvs.clear_wifi_credentials()?;
        self.state = WifiState::Unconfigured;
        log::info!("WiFi凭证已清除，下次启动将进入配网模式");
        Ok(())
    }

    pub fn state(&self) -> WifiState {
        self.state
    }

    pub fn is_configured(&self) -> bool {
        self.nvs.has_wifi_credentials()
    }
}
