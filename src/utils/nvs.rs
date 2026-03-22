use esp_idf_svc::nvs::{EspNvs, EspNvsPartition, NvsDefault};
use esp_idf_svc::sys::EspError;

use crate::utils::config::WifiCredentials;

const NVS_NAMESPACE: &str = "wifi_config";
const KEY_WIFI_SSID: &str = "ssid";
const KEY_WIFI_PASS: &str = "password";

pub struct NvsStorage {
    nvs: EspNvs<NvsDefault>,
}

impl NvsStorage {
    pub fn new() -> Result<Self, EspError> {
        let nvs_partition = EspNvsPartition::<NvsDefault>::take()?;
        let nvs = EspNvs::new(nvs_partition, NVS_NAMESPACE, true)?;
        Ok(Self { nvs })
    }

    pub fn save_wifi_credentials(&mut self, creds: &WifiCredentials) -> Result<(), EspError> {
        self.nvs.set_str(KEY_WIFI_SSID, &creds.ssid)?;
        self.nvs.set_str(KEY_WIFI_PASS, &creds.password)?;
        log::info!("WiFi凭证已保存到NVS");
        Ok(())
    }

    pub fn load_wifi_credentials(&self) -> Result<Option<WifiCredentials>, EspError> {
        let mut ssid_buf = [0u8; 64];
        let ssid = match self.nvs.get_str(KEY_WIFI_SSID, &mut ssid_buf)? {
            Some(s) => s.to_string(),
            None => return Ok(None),
        };

        let mut pass_buf = [0u8; 64];
        let password = match self.nvs.get_str(KEY_WIFI_PASS, &mut pass_buf)? {
            Some(s) => s.to_string(),
            None => String::new(),
        };

        Ok(Some(WifiCredentials { ssid, password }))
    }

    pub fn clear_wifi_credentials(&mut self) -> Result<(), EspError> {
        self.nvs.remove(KEY_WIFI_SSID)?;
        self.nvs.remove(KEY_WIFI_PASS)?;
        log::info!("WiFi凭证已清除");
        Ok(())
    }

    pub fn has_wifi_credentials(&self) -> bool {
        let mut buf = [0u8; 64];
        self.nvs.get_str(KEY_WIFI_SSID, &mut buf).ok().flatten().is_some()
    }
}
