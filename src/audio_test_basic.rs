use esp_idf_svc::sys::link_patches;
use esp_idf_svc::log::EspLogger;
use esp_idf_svc::hal::delay::Delay;
use esp_idf_svc::hal::gpio::{Output, PinDriver};
use esp_idf_svc::hal::peripherals::Peripherals;
use log::info;

fn main() -> anyhow::Result<()> {
    link_patches();
    EspLogger::initialize_default();

    info!("========================================");
    info!("🎙️ 基础音频测试程序启动...");
    info!("========================================");

    let peripherals = Peripherals::take().unwrap();
    
    // 使用GPIO7直接驱动扬声器（简单的方波测试）
    let mut speaker = PinDriver::output(peripherals.pins.gpio7)?;
    info!("GPIO7输出配置成功");

    // 播放简单的方波测试音
    info!("开始播放简单测试音...");
    
    let mut delay = Delay::new_default();
    
    // 测试不同频率的方波
    let frequencies = [400, 600, 800, 1000, 1200];
    
    for &freq in &frequencies {
        info!("播放频率: {} Hz", freq);
        
        // 播放100ms
        for _ in 0..(freq / 10) { // 大概播放100ms
            speaker.set_high()?;
            delay.delay_us(500_000 / freq); // 半周期
            speaker.set_low()?;
            delay.delay_us(500_000 / freq);
        }
        
        delay.delay_ms(200); // 暂停200ms
    }

    info!("========================================");
    info!("🎙️ 基础音频测试完成");
    info!("========================================");
    info!("如果没听到声音，请检查硬件连接");
    
    loop {
        delay.delay_ms(1000);
        info!("程序运行中...");
    }
}