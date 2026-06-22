use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use esp_idf_svc::hal::i2s::config::{DataBitWidth, StdConfig};
use esp_idf_svc::hal::i2s::{I2sDriver, I2sTx};
use esp_idf_svc::hal::peripherals::Peripherals;
use log::{error, info};

#[derive(Clone, Copy)]
pub struct SpeakerServiceConfig {
    pub config: SpeakerConfig,
    pub buffer_size: usize,
    pub queue_capacity: usize,
}

impl Default for SpeakerServiceConfig {
    fn default() -> Self {
        Self {
            config: SpeakerConfig::default(),
            buffer_size: 512,
            queue_capacity: 32,
        }
    }
}
#[derive(Clone, Copy)]
pub struct SpeakerConfig {
    pub sample_rate: u32,
    pub bclk_pin: i32,
    pub din_pin: i32,
    pub ws_pin: i32,
}

impl Default for SpeakerConfig {
    fn default() -> Self {
        Self {
            sample_rate: 16000,
            bclk_pin: 15,
            din_pin: 7,
            ws_pin: 16,
        }
    }
}

enum SoundCommand {
    Play {
        audio: Vec<u8>,
        pcm_format: PcmFormat,
    },
    Stopping,
}

pub struct SpeakerService {
    config: Arc<SpeakerServiceConfig>,
    stop_signal: Arc<AtomicBool>,
    worker_thread: Option<thread::JoinHandle<()>>,
    sender: mpsc::SyncSender<SoundCommand>,
}

impl SpeakerService {
    pub fn new(service_config: SpeakerServiceConfig) -> Result<Self, SpeakerError> {
        info!("========================================");
        info!("🔊 音响程序启动...");
        info!("========================================");
        info!(
            "音响配置: 采样率={}Hz, 缓冲区大小={}, 队列容量={}",
            service_config.config.sample_rate,
            service_config.buffer_size,
            service_config.queue_capacity
        );
        info!(
            "I2S引脚配置: BCLK={}, DIN={}, WS={}",
            service_config.config.bclk_pin,
            service_config.config.din_pin,
            service_config.config.ws_pin
        );

        let peripherals = Peripherals::take().map_err(|e| {
            error!("❌ 获取外设失败: {:?}", e);
            SpeakerError::PeripheralError
        })?;

        let config = service_config.config;

        info!("初始化 I2S 音频输出...");
        let i2s_config = StdConfig::philips(config.sample_rate, DataBitWidth::Bits16);
        info!(
            "I2S配置: 标准模式, 采样率={}Hz, 位宽=16位",
            config.sample_rate
        );

        info!("正在创建 I2sDriver 实例...");
        let mut i2s = I2sDriver::<I2sTx>::new_std_tx(
            peripherals.i2s1,
            &i2s_config,
            peripherals.pins.gpio15,               // BCLK (Bit Clock)
            peripherals.pins.gpio7,                // DIN (Data Input)
            None::<esp_idf_svc::hal::gpio::Gpio0>, // MCLK（不使用）
            peripherals.pins.gpio16,               // WS (Word Select)
        )
        .map_err(|e| {
            error!("❌ 创建 I2sDriver 失败: {:?}", e);
            SpeakerError::from(e)
        })?;

        // 启用 I2S 发送通道
        i2s.tx_enable().map_err(|e| {
            error!("❌ 启用 I2S 发送通道失败: {:?}", e);
            SpeakerError::from(e)
        })?;

        info!("✅ I2S 音频输出初始化成功");

        let (sender, receiver) = mpsc::sync_channel::<SoundCommand>(service_config.queue_capacity);
        let stop_signal = Arc::new(AtomicBool::new(false));
        let stop_signal_clone = stop_signal.clone();
        let buffer_size = service_config.buffer_size;
        let worker_thread =
            thread::spawn(move || Self::run_work(i2s, receiver, stop_signal_clone, buffer_size));

        Ok(Self {
            config: Arc::new(service_config),
            stop_signal,
            worker_thread: Some(worker_thread),
            sender,
        })
    }

    fn run_work(
        mut i2s: I2sDriver<'static, I2sTx>,
        receiver: mpsc::Receiver<SoundCommand>,
        stop_signal: Arc<AtomicBool>,
        buffer_size: usize,
    ) {
        info!("正在启动音频播放线程...");
        while !stop_signal.load(std::sync::atomic::Ordering::Relaxed) {
            match receiver.recv_timeout(Duration::from_millis(100)) {
                Ok(cmd) => {
                    match cmd {
                        SoundCommand::Play { audio, pcm_format } => {
                            info!("收到音频数据，长度: {} 字节", audio.len());
                            // 将音频数据分块写入
                            let mut offset = 0;
                            while offset < audio.len() {
                                let remaining = audio.len() - offset;
                                let chunk_size = std::cmp::min(buffer_size, remaining);

                                match i2s.write(&audio[offset..offset + chunk_size], 1000) {
                                    Ok(sent) => {
                                        offset += sent;
                                    }
                                    Err(e) => {
                                        error!("❌ 写入音频数据失败: {:?}", e);
                                        break;
                                    }
                                }
                            }

                            info!("音频数据播放完成");
                        }
                        SoundCommand::Stopping => {
                            info!("收到停止指令");
                            break;
                        }
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    // 队列为空，继续等待
                    continue;
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    info!("音频队列已断开连接");
                    break;
                }
            }
        }
        let _ = i2s.tx_disable();
        stop_signal.store(true, std::sync::atomic::Ordering::Relaxed);
        info!("音频播放线程退出");
    }

    pub fn is_stoped(&self) -> bool {
        self.stop_signal.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn check_stoped(&self) -> Result<(), SpeakerError> {
        if self.is_stoped() {
            return Err(SpeakerError::Stopped);
        }
        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), SpeakerError> {
        if self.is_stoped() {
            return Ok(());
        }
        info!("开始停止音响服务...");
        self.stop_signal
            .store(true, std::sync::atomic::Ordering::Relaxed);
        let _ = self.sender.send(SoundCommand::Stopping);
        self.worker_thread.take().map(|work_t| work_t.join());
        info!("✅ 音响服务已停止");
        Ok(())
    }

    pub fn play_audio_data(
        &self,
        audio_data: &[u8],
        format: PcmFormat,
    ) -> Result<(), SpeakerError> {
        if audio_data.is_empty() {
            return Ok(());
        }
        self.check_stoped()?;
        self.sender
            .send(SoundCommand::Play {
                audio: audio_data.to_vec(),
                pcm_format: format,
            })
            .map_err(|e| SpeakerError::QueueError(e.to_string()))?;
        Ok(())
    }
}

impl Drop for SpeakerService {
    fn drop(&mut self) {
        self.stop();
    }
}

/// PCM 数据格式
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PcmFormat {
    Pcm,      // PcmF32le
    PcmS16le, // PcmS16le
}

impl PcmFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pcm => "pcm",
            Self::PcmS16le => "pcm_s16le",
        }
    }
}

impl From<&str> for PcmFormat {
    fn from(s: &str) -> Self {
        match s {
            "pcm" => Self::Pcm,
            "pcm_s16le" => Self::PcmS16le,
            _ => panic!("Invalid PCM format: {}", s),
        }
    }
}

pub enum SpeakerError {
    Stopped,
    I2SError(esp_idf_svc::sys::EspError),
    PeripheralError,
    NotInitialized,
    QueueError(String),
}

impl std::fmt::Debug for SpeakerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SpeakerError::Stopped => write!(f, "已停止"),
            SpeakerError::I2SError(e) => write!(f, "I2S错误: {:?}", e),
            SpeakerError::PeripheralError => write!(f, "外设错误"),
            SpeakerError::NotInitialized => write!(f, "未初始化"),
            SpeakerError::QueueError(e) => write!(f, "队列错误: {}", e),
        }
    }
}

impl std::fmt::Display for SpeakerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SpeakerError::Stopped => write!(f, "已停止"),
            SpeakerError::I2SError(e) => write!(f, "I2S错误: {:?}", e),
            SpeakerError::PeripheralError => write!(f, "外设错误"),
            SpeakerError::NotInitialized => write!(f, "未初始化"),
            SpeakerError::QueueError(e) => write!(f, "队列错误: {}", e),
        }
    }
}

impl std::error::Error for SpeakerError {}

impl From<esp_idf_svc::sys::EspError> for SpeakerError {
    fn from(err: esp_idf_svc::sys::EspError) -> Self {
        SpeakerError::I2SError(err)
    }
}
