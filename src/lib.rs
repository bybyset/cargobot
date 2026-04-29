// 导出项目的公共模块
pub mod utils;
pub mod wifi;
pub mod audio;
pub mod oled;
pub mod error;
pub mod file_storage;

pub use crate::file_storage::FileSpiffsStorage;




// 重新导出主要功能
pub use crate::audio::simple_buzzer;
pub use crate::audio::rtvoice;