use std::{sync::Arc, thread, time::Duration};

use esp_idf_svc::sys::link_patches;
use esp_idf_svc::{hal::peripherals::Peripherals, log::EspLogger};
use log::info;

mod audio;
mod error;
mod file_storage;
mod oled;
mod utils;
mod wifi;

use crate::audio::{
    rtvoice::doubao::{config::RuntimeConfig, RtService},
    MicrophoneService, MicrophoneServiceConfig, SpeakerServiceConfig, WakeWordService,
};

fn main() -> anyhow::Result<()> {
    link_patches();
    EspLogger::initialize_default();

    info!("========================================");
    info!("🎙️ 小梁语音助手启动中...");
    info!("========================================");
    let peripherals =
        Peripherals::take().map_err(|e| anyhow::anyhow!("❌ 获取外设失败: {:?}", e))?;

    // 引导设置WiFi
    let wifi_manager = wifi::wait_for_wifi_connection(peripherals.modem)?;

    // 启动音响服务
    let speaker_service = audio::SpeakerService::new(
        SpeakerServiceConfig::default(),
        peripherals.i2s1,
        peripherals.pins.gpio15,
        peripherals.pins.gpio7,
        None::<esp_idf_svc::hal::gpio::Gpio0>,
        peripherals.pins.gpio16,
    )?;
    let speaker_service = Arc::new(speaker_service);
    
    // 启动麦克风服务
    let microphone_service = audio::MicrophoneService::new(
        MicrophoneServiceConfig::default(),
        peripherals.i2s0,
        peripherals.pins.gpio5,
        peripherals.pins.gpio6,
        None::<esp_idf_svc::hal::gpio::Gpio0>,
        peripherals.pins.gpio4,
    )?;
    let microphone_service = Arc::new(microphone_service);
    let microphone_service_clone = microphone_service.clone();
    // 开启豆包语音服务
    let doubao_config = RuntimeConfig::new_from_keys(
        String::from("xx"),
        String::from("xx"),
        String::from("xx"),
    );
    let mut rt_service = audio::rtvoice::doubao::RtService::new(
        speaker_service,
        microphone_service_clone,
        doubao_config,
    );

    // 监听唤醒词，若检测到则开启豆包语音服务
    let wake_service = start_wake_word_service(microphone_service, rt_service)?;

    info!("系统初始化完成，进入主循环...");

    thread::park();
    info!("系统退出!");
    Ok(())
}

fn start_wake_word_service(
    microphone_service: Arc<MicrophoneService>,
    mut rt_service: RtService,
) -> anyhow::Result<WakeWordService> {
    // 挂载 SPIFFS 分区
    file_storage::mount_spiffs().map_err(|e| anyhow::anyhow!("挂载 SPIFFS 失败"))?;
    // 列出 SPIFFS storage 目录中的文件
    info!("========================================");
    info!("📁 SPIFFS /storage 目录内容:");
    info!("========================================");
    match std::fs::read_dir("/storage") {
        Ok(entries) => {
            let mut found_model = false;
            for entry in entries {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if path.is_file() {
                        let filename = path.file_name().unwrap_or_default().to_string_lossy();
                        info!("📄 {}", filename);
                        if filename.contains("nihaoxiaoliang") {
                            found_model = true;
                        }
                    }
                }
            }
            if !found_model {
                info!("⚠️ 未找到唤醒词模型文件: nihaoxiaoliang.rpw");
                info!("💡 请先将模型文件烧录到 SPIFFS 分区");
            }
        }
        Err(e) => {
            info!("❌ 无法读取 /storage 目录: {:?}", e);
            info!("💡 请确保 SPIFFS 分区已正确挂载");
        }
    }

    // 启动唤醒词服务
    let wake_word_config = audio::WakeWordConfig::default();
    let microphone_service_clone = microphone_service.clone();
    let wake_service = audio::WakeWordService::new(
        wake_word_config,
        microphone_service_clone,
        move |wake_word| {
            println!("唤醒词: {}", wake_word);
            let start_result = rt_service.start();
            if let Err(e) = start_result {
                println!("start_result: {:?}", e);
            } else {
                println!("start rtvoice success");
                thread::sleep(Duration::from_secs(300));
                rt_service.stop();
            }
        },
    )?;
    Ok(wake_service)
}
