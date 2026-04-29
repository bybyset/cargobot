use esp_idf_svc::sys::link_patches;
use esp_idf_svc::log::EspLogger;
use esp_idf_svc::hal::peripherals::Peripherals;
use esp_idf_svc::hal::i2s::config::{DataBitWidth, StdConfig};
use esp_idf_svc::hal::i2s::I2sDriver;
use esp_idf_svc::hal::i2s::I2sRx;
use esp_idf_svc::hal::delay::Delay;
use log::{info, error, debug};
mod file_storage;
mod error;

const SAMPLE_RATE: u32 = 16000; // 16kHz 采样率
const BUFFER_SIZE: usize = 1024; // 1KB 缓冲区
const THRESHOLD: i16 = 500; // 声音检测阈值
const READ_TIMEOUT: u32 = 100; // 读取超时时间（ms）

fn main() -> anyhow::Result<()> {
    link_patches();
    EspLogger::initialize_default();

    info!("========================================");
    info!("🎤 麦克风测试程序启动...");
    info!("========================================");

    let peripherals = Peripherals::take()?;

    // 配置 I2S 音频输入（麦克风）- 标准模式
    info!("初始化 I2S 音频输入...");
    let i2s_config = StdConfig::philips(SAMPLE_RATE, DataBitWidth::Bits16);
    
    debug!("正在创建 I2sDriver 实例...");
    let mut i2s = I2sDriver::<I2sRx>::new_std_rx(
        peripherals.i2s0,
        &i2s_config,
        peripherals.pins.gpio5,  // SCK (Serial Clock)
        peripherals.pins.gpio6,  // SD (Serial Data)
        None::<esp_idf_svc::hal::gpio::Gpio0>, // MCLK（不使用）
        peripherals.pins.gpio4,  // WS (Word Select)
    )?;

    info!("✅ I2S 音频输入初始化成功");
    info!("采样率: 16kHz");
    info!("缓冲区大小: 1024 字节");
    info!("声音检测阈值: 500");
    info!("读取超时: 100ms");

    debug!("正在启用 I2S 接收通道...");
    match i2s.rx_enable() {
        Ok(()) => info!("✅ I2S 接收通道启用成功"),
        Err(e) => {
            error!("❌ 启用 I2S 接收通道失败: {:?}", e);
            return Err(e.into());
        }
    }

    let mut buffer = vec![0u8; BUFFER_SIZE * 2]; // 16位采样，所以每个样本占2字节
    let mut delay = Delay::new_default();
    let mut sound_detected = false;

    info!("========================================");
    info!("开始麦克风测试...");
    info!("对着麦克风说话或发出声音来测试");
    info!("========================================");

    loop {
        // 读取音频数据
        match i2s.read(&mut buffer, READ_TIMEOUT) {
            Ok(count) => {
                if count > 0 {
                    debug!("成功读取 {} 字节音频数据", count);
                    
                    // 将字节转换为 i16 样本
                    let mut samples = Vec::with_capacity(count / 2);
                    let mut i = 0;
                    while i + 1 < count {
                        let sample = (buffer[i + 1] as i16) << 8 | (buffer[i] as i16);
                        samples.push(sample);
                        i += 2;
                    }

                    debug!("解析到 {} 个 i16 样本", samples.len());

                    // 计算音量（RMS值）
                    let mut sum_squares = 0i32;
                    for &sample in &samples {
                        sum_squares += (sample as i32).pow(2);
                    }
                    let rms = ((sum_squares / samples.len() as i32) as f32).sqrt() as i16;

                    // 检测声音
                    if rms > THRESHOLD {
                        if !sound_detected {
                            sound_detected = true;
                            info!("🎵 检测到声音！音量: {}", rms);
                        }
                    } else {
                        if sound_detected {
                            sound_detected = false;
                            info!("🔇 声音停止");
                        }
                    }

                    // 输出音频数据统计信息（每500ms）
                    static mut LAST_TIME: u32 = 0;
                    let current_time = unsafe { esp_idf_svc::sys::xTaskGetTickCount() };
                    unsafe {
                        if current_time - LAST_TIME > 500 {
                            LAST_TIME = current_time;
                            let max_sample = samples.iter().map(|&s| s.abs()).max().unwrap_or(0);
                            let min_sample = samples.iter().map(|&s| s.abs()).min().unwrap_or(0);
                            let avg_sample = samples.iter().map(|&s| s as i32).sum::<i32>() / samples.len() as i32;

                            info!(
                                "📊 音频统计 - 音量: {}, 最大值: {}, 最小值: {}, 平均值: {}",
                                rms, max_sample, min_sample, avg_sample
                            );
                        }
                    }
                } else {
                    debug!("读取到 0 字节音频数据（超时）");
                }
            }
            Err(e) => {
                error!("❌ 读取音频数据失败: {:?}", e);
                debug!("I2S 错误详情: code={}, description={}", e.code(), e);
            }
        }

        delay.delay_ms(100);
    }
}
