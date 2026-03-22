use esp_idf_svc::sys::link_patches;
use esp_idf_svc::log::EspLogger;
use log::info;

mod utils;
mod wifi;

use wifi::manager::WifiManager;

fn main() -> anyhow::Result<()> {
    link_patches();
    EspLogger::initialize_default();

    info!("========================================");
    info!("🎙️ 小梁语音助手启动中...");
    info!("========================================");

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

    if let Err(e) = wifi_manager.ensure_connected() {
        log::error!("WiFi连接失败: {}", e);
    }

    info!("系统初始化完成，进入主循环...");

    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}
