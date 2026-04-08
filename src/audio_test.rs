use esp_idf_svc::sys::link_patches;
use esp_idf_svc::log::EspLogger;
use log::info;

mod audio;
fn main() -> anyhow::Result<()> {
    link_patches();
    EspLogger::initialize_default();

    info!("========================================");
    info!("🔊 MAX98357A 喇叭测试程序启动...");
    info!("========================================");

    info!("正在初始化 SimpleBuzzer...");
    let mut buzzer = audio::simple_buzzer::SimpleBuzzer::new()?;

    info!("✅ SimpleBuzzer 初始化成功");
    info!("========================================");
    info!("开始喇叭测试...");
    info!("如果听到声音，说明喇叭和 I2S 接口工作正常");
    info!("========================================");

    // 播放测试音
    info!("播放测试音序列...");
    buzzer.play_test_sound()?;
    
    info!("📊 喇叭测试完成！");

    // 等待一段时间后退出
    std::thread::sleep(std::time::Duration::from_secs(2));

    Ok(())
}
