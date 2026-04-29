use cargobot::file_storage;
use cargobot::audio::{MicrophoneService, MicrophoneConfig, WakeWordService, WakeWordConfig};
use esp_idf_svc::sys::link_patches;
use esp_idf_svc::log::EspLogger;
use log::{info, error};

fn main() -> anyhow::Result<()> {
    link_patches();
    EspLogger::initialize_default();

    info!("========================================");
    info!("🎯 唤醒词检测 Demo");
    info!("========================================");

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

    // 在单独的线程中创建和启动所有服务
    info!("========================================");
    info!("🚀 启动服务线程...");
    info!("========================================");
    
    std::thread::Builder::new()
        .stack_size(65536) // 增加栈空间到64KB，确保模型加载有足够空间
        .name("service_thread".to_string())
        .spawn(|| {
            info!("服务线程启动，开始初始化所有服务...");
            
            // 1. 初始化唤醒词服务（先初始化，不依赖麦克风）
            info!("========================================");
            info!("🔊 初始化唤醒词服务...");
            info!("========================================");
            let wakeword_config = WakeWordConfig::default();
            match WakeWordService::new(wakeword_config) {
                Ok(mut wakeword_service) => {
                    info!("✅ 唤醒词服务创建成功");
                    
                    // 2. 初始化麦克风服务
                    info!("========================================");
                    info!("🎤 初始化麦克风服务...");
                    info!("========================================");
                    let microphone_config = MicrophoneConfig::default();
                    match MicrophoneService::new(microphone_config) {
                        Ok(mut microphone_service) => {
                            info!("✅ 麦克风服务创建成功");
                            
                            // 3. 生成音频消费者
                            match microphone_service.gen_audio_consumer() {
                                Ok(audio_consumer) => {
                                    info!("✅ 音频消费者创建成功");
                                    
                                    // 4. 启动所有服务
                                    info!("========================================");
                                    info!("🚀 启动所有服务...");
                                    info!("========================================");
                                    
                                    // 启动麦克风服务
                                    if let Err(e) = microphone_service.start() {
                                        error!("❌ 麦克风服务启动失败: {:?}", e);
                                        return;
                                    }
                                    info!("✅ 麦克风服务已启动");
                                    
                                    // 启动唤醒词检测
                                    if let Err(e) = wakeword_service.start(audio_consumer, |wakeword| {
                                        info!("========================================");
                                        info!("🎉 检测到唤醒词: {}", wakeword);
                                        info!("========================================");
                                    }) {
                                        error!("❌ 唤醒词检测启动失败: {}", e);
                                        return;
                                    }
                                    info!("✅ 唤醒词检测服务已启动");
                                    
                                    info!("========================================");
                                    info!("✅ 所有服务启动成功！");
                                    info!("💡 请对着麦克风说唤醒词进行测试");
                                    info!("========================================");
                                }
                                Err(e) => {
                                    error!("❌ 音频消费者生成失败: {:?}", e);
                                }
                            }
                        }
                        Err(e) => {
                            error!("❌ 麦克风服务初始化失败: {:?}", e);
                        }
                    }
                }
                Err(e) => {
                    error!("❌ 唤醒词服务初始化失败: {:?}", e);
                }
            }
        })
        .map_err(|e| anyhow::anyhow!("创建服务线程失败: {:?}", e))?;

    info!("========================================");
    info!("✅ 系统启动成功！");
    info!("💡 请对着麦克风说唤醒词进行测试");
    info!("========================================");

    // 保持主线程运行
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}