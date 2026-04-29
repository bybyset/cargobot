use std::fs::File;
use std::ffi::CString;
use esp_idf_svc::sys::{EspError, esp, esp_vfs_spiffs_conf_t, esp_vfs_spiffs_register};

use crate::error::BotError;

pub mod config;

pub struct FileSpiffsStorage {
    mount_name: &'static str,
}

impl FileSpiffsStorage {
    pub fn new(mount_name: &'static str) -> Self {
        Self { mount_name }
    }

    pub fn open_file(&self, file_name: &str) -> std::io::Result<File> {
        let file_path = format!("{}/{}", self.mount_name, file_name);
        File::open(file_path)
    }

    pub fn get_file_path(&self, file_name: &str) -> String {
        format!("{}/{}", self.mount_name, file_name)
    }
}




pub fn mount_spiffs() -> Result<(), BotError> {
    let partition_label = CString::new(config::PARTITION_NAME).map_err(|e| BotError::CStringError(e))?;
    let mount_point = CString::new(config::MOUNT_NAME).map_err(|e| BotError::CStringError(e))?;
    
    let config = esp_vfs_spiffs_conf_t {
        base_path: mount_point.as_ptr(),
        partition_label: partition_label.as_ptr(),
        max_files: 5,
        format_if_mount_failed: false,
        ..Default::default()
    };
    
    esp!(unsafe { esp_vfs_spiffs_register(&config) }).map_err(|e| BotError::EspError(e))?;
    Ok(())
}