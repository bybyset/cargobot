use esp_idf_svc::sys::link_patches;
use esp_idf_svc::log::EspLogger;
use esp_idf_svc::hal::delay::Delay;
use log::{info, error};

mod oled;
use oled::driver::OledDisplay;
mod file_storage;
mod error;

fn main() -> anyhow::Result<()> {
    link_patches();
    EspLogger::initialize_default();

    info!("========================================");
    info!("📺 OLED显示屏测试程序启动...");
    info!("========================================");

    // 创建OLED驱动
    info!("正在初始化OLED驱动...");
    let mut display = match OledDisplay::new() {
        Ok(d) => {
            info!("✅ OLED驱动初始化成功");
            d
        },
        Err(e) => {
            error!("❌ OLED驱动初始化失败: {:?}", e);
            info!("========================================");
            info!("程序启动失败，正在退出...");
            info!("========================================");
            return Err(anyhow::anyhow!("OLED驱动初始化失败: {:?}", e));
        }
    };

    // 测试OLED显示功能
    info!("正在测试OLED显示功能...");
    match display.test_display() {
        Ok(_) => {
            info!("✅ OLED显示测试成功");
        },
        Err(e) => {
            error!("❌ OLED显示测试失败: {:?}", e);
        }
    }
    
    // 等待2秒
    Delay::new_default().delay_ms(2000);

    // 清除屏幕，准备下一个测试
    info!("正在清除显示缓冲区...");
    match display.clear() {
        Ok(_) => info!("✅ 显示缓冲区清除成功"),
        Err(e) => error!("❌ 显示缓冲区清除失败: {:?}", e),
    }

    // 显示硬件信息
    info!("正在显示硬件信息...");
    display.draw_string(0, 0, "ESP32-S3");
    display.draw_string(0, 16, "OLED Test");
    display.draw_string(0, 32, "I2C: 0x3C");
    display.draw_string(0, 48, "GPIO41/42");
    match display.update() {
        Ok(_) => info!("✅ 硬件信息显示成功"),
        Err(e) => error!("❌ 硬件信息显示失败: {:?}", e),
    }

    info!("========================================");
    info!("✅ OLED显示测试完成");
    info!("========================================");
    info!("如果显示屏未显示，请检查硬件连接");
    info!("可以尝试:");
    info!("1. 检查SDA/GND连接");
    info!("2. 确保I2C地址是0x3C");
    info!("3. 检查显示屏电源");
    info!("4. 重新启动程序");

    // 主循环，保持程序运行
    loop {
        Delay::new_default().delay_ms(1000);
        info!("程序运行中...");
    }
}