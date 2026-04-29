use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::mpsc::SyncSender;
use std::thread;
use std::time::Duration;

use esp_idf_svc::hal::delay::Delay;
use esp_idf_svc::hal::i2s::config::{DataBitWidth, StdConfig};
use esp_idf_svc::hal::i2s::{I2sDriver, I2sTx};
use esp_idf_svc::hal::peripherals::Peripherals;
use log::{error, info};

pub struct SpeakerConfig {
    pub sample_rate: u32,
    pub buffer_size: usize,
    pub queue_capacity: usize,
    pub bclk_pin: i32,
    pub din_pin: i32,
    pub ws_pin: i32,
}

impl Default for SpeakerConfig {
    fn default() -> Self {
        Self {
            sample_rate: 16000,
            buffer_size: 512,
            queue_capacity: 32,
            bclk_pin: 15,
            din_pin: 7,
            ws_pin: 16,
        }
    }
}

pub struct SpeakerService {
    config: SpeakerConfig,
    i2s: Option<I2sDriver<'static, I2sTx>>,
    stop_signal: Arc<AtomicBool>,
    worker_thread: Option<thread::JoinHandle<()>>,
    sender: Option<SyncSender<Vec<u8>>>,
}

impl SpeakerService {
    pub fn new(config: SpeakerConfig) -> Result<Self, SpeakerError> {
        info!("========================================");
        info!("🔊 音响程序启动...");
        info!("========================================");
        info!("音响配置: 采样率={}Hz, 缓冲区大小={}, 队列容量={}", 
              config.sample_rate, config.buffer_size, config.queue_capacity);
        info!("I2S引脚配置: BCLK={}, DIN={}, WS={}", 
              config.bclk_pin, config.din_pin, config.ws_pin);

        let peripherals = Peripherals::take()
            .map_err(|e| {
                error!("❌ 获取外设失败: {:?}", e);
                SpeakerError::PeripheralError
            })?;

        info!("初始化 I2S 音频输出...");
        let i2s_config = StdConfig::philips(config.sample_rate, DataBitWidth::Bits16);
        info!("I2S配置: 标准模式, 采样率={}Hz, 位宽=16位", config.sample_rate);

        info!("正在创建 I2sDriver 实例...");
        let i2s = I2sDriver::<I2sTx>::new_std_tx(
            peripherals.i2s1,
            &i2s_config,
            peripherals.pins.gpio15,                // BCLK (Bit Clock)
            peripherals.pins.gpio7,                 // DIN (Data Input)
            None::<esp_idf_svc::hal::gpio::Gpio0>, // MCLK（不使用）
            peripherals.pins.gpio16,                // WS (Word Select)
        ).map_err(|e| {
            error!("❌ 创建 I2sDriver 失败: {:?}", e);
            SpeakerError::from(e)
        })?;

        info!("✅ I2S 音频输出初始化成功");

        Ok(Self {
            config,
            i2s: Some(i2s),
            stop_signal: Arc::new(AtomicBool::new(false)),
            worker_thread: None,
            sender: None,
        })
    }

    pub fn start(&mut self) -> Result<(), SpeakerError> {
        info!("开始启动音响服务...");
        
        let mut i2s = self.i2s.take().ok_or_else(|| {
            error!("❌ 音响服务未初始化");
            SpeakerError::NotInitialized
        })?;

        info!("正在启用 I2S 发送通道...");
        match i2s.tx_enable() {
            Ok(()) => info!("✅ I2S 发送通道启用成功"),
            Err(e) => {
                error!("❌ 启用 I2S 发送通道失败: {:?}", e);
                self.i2s = Some(i2s);
                return Err(e.into());
            }
        }

        // 创建阻塞队列
        let (sender, receiver) = mpsc::sync_channel(self.config.queue_capacity);
        self.sender = Some(sender);

        // 启动播放线程
        self.start_playback_thread(i2s, receiver)?;

        info!("✅ 音响服务启动成功");
        Ok(())
    }

    fn start_playback_thread(
        &mut self,
        mut i2s: I2sDriver<'static, I2sTx>,
        receiver: mpsc::Receiver<Vec<u8>>,
    ) -> Result<(), SpeakerError> {
        info!("正在启动音频播放线程...");
        
        let stop_signal = self.stop_signal.clone();
        let buffer_size = self.config.buffer_size;
        
        let thread = thread::spawn(move || {
            info!("音频播放线程已启动");
            
            while !stop_signal.load(std::sync::atomic::Ordering::Relaxed) {
                match receiver.recv_timeout(Duration::from_millis(100)) {
                    Ok(audio_data) => {
                        info!("收到音频数据，长度: {} 字节", audio_data.len());
                        
                        // 将音频数据分块写入
                        let mut offset = 0;
                        while offset < audio_data.len() {
                            let remaining = audio_data.len() - offset;
                            let chunk_size = std::cmp::min(buffer_size, remaining);
                            
                            match i2s.write(&audio_data[offset..offset + chunk_size], 1000) {
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
            
            info!("音频播放线程退出");
        });
        
        self.worker_thread = Some(thread);
        info!("✅ 音频播放线程启动成功");
        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), SpeakerError> {
        info!("开始停止音响服务...");
        self.stop_signal
            .store(true, std::sync::atomic::Ordering::Relaxed);

        // 关闭发送端，让接收端知道没有更多数据
        self.sender = None;

        if let Some(thread) = self.worker_thread.take() {
            info!("等待音频播放线程退出...");
            if let Err(e) = thread.join() {
                error!("❌ 线程 join 失败: {:?}", e);
            } else {
                info!("✅ 音频播放线程已退出");
            }
        }

        info!("✅ 音响服务已停止");
        Ok(())
    }

    pub fn play_audio_data(&self, audio_data: &[u8]) -> Result<(), SpeakerError> {
        if audio_data.is_empty() {
            return Ok(());
        }

        let sender = self.sender.as_ref().ok_or(SpeakerError::NotInitialized)?;
        
        match sender.send(audio_data.to_vec()) {
            Ok(()) => {
                info!("音频数据已发送到播放队列: {} 字节", audio_data.len());
                Ok(())
            }
            Err(e) => {
                error!("❌ 发送音频数据失败: {:?}", e);
                Err(SpeakerError::QueueError(e.to_string()))
            }
        }
    }

    pub fn play_tone(&self, frequency: u32, duration_ms: u32) -> Result<(), SpeakerError> {
        info!("播放单音: 频率={}Hz, 持续时间={}ms", frequency, duration_ms);

        let samples_count = (self.config.sample_rate * duration_ms / 1000) as usize;
        let mut buffer = vec![0u8; samples_count * 2];

        for i in 0..samples_count {
            let t = i as f32 / self.config.sample_rate as f32;
            let value = (f32::sin(2.0 * core::f32::consts::PI * frequency as f32 * t) * 0.3 * i16::MAX as f32) as i16;
            let offset = i * 2;
            buffer[offset] = (value & 0xFF) as u8;
            buffer[offset + 1] = (value >> 8) as u8;
        }

        self.play_audio_data(&buffer)
    }

    pub fn play_beep(&self) -> Result<(), SpeakerError> {
        self.play_tone(1000, 200)
    }

    pub fn play_success_sound(&self) -> Result<(), SpeakerError> {
        info!("播放成功提示音...");
        
        let mut audio_data = Vec::new();
        
        // 400Hz, 150ms
        audio_data.extend(self.generate_tone(400, 150));
        
        // 100ms silence
        audio_data.extend(vec![0u8; (self.config.sample_rate * 100 / 1000) as usize * 2]);
        
        // 600Hz, 150ms
        audio_data.extend(self.generate_tone(600, 150));
        
        // 100ms silence
        audio_data.extend(vec![0u8; (self.config.sample_rate * 100 / 1000) as usize * 2]);
        
        // 800Hz, 300ms
        audio_data.extend(self.generate_tone(800, 300));
        
        self.play_audio_data(&audio_data)
    }

    pub fn play_error_sound(&self) -> Result<(), SpeakerError> {
        info!("播放错误提示音...");
        
        let mut audio_data = Vec::new();
        
        // 800Hz, 200ms
        audio_data.extend(self.generate_tone(800, 200));
        
        // 100ms silence
        audio_data.extend(vec![0u8; (self.config.sample_rate * 100 / 1000) as usize * 2]);
        
        // 400Hz, 400ms
        audio_data.extend(self.generate_tone(400, 400));
        
        self.play_audio_data(&audio_data)
    }

    pub fn play_notification_sound(&self) -> Result<(), SpeakerError> {
        info!("播放通知提示音...");
        
        let mut audio_data = Vec::new();
        
        // 600Hz, 100ms
        audio_data.extend(self.generate_tone(600, 100));
        
        // 50ms silence
        audio_data.extend(vec![0u8; (self.config.sample_rate * 50 / 1000) as usize * 2]);
        
        // 800Hz, 100ms
        audio_data.extend(self.generate_tone(800, 100));
        
        // 50ms silence
        audio_data.extend(vec![0u8; (self.config.sample_rate * 50 / 1000) as usize * 2]);
        
        // 1000Hz, 200ms
        audio_data.extend(self.generate_tone(1000, 200));
        
        self.play_audio_data(&audio_data)
    }

    fn generate_tone(&self, frequency: u32, duration_ms: u32) -> Vec<u8> {
        let samples_count = (self.config.sample_rate * duration_ms / 1000) as usize;
        let mut buffer = Vec::with_capacity(samples_count * 2);

        for i in 0..samples_count {
            let t = i as f32 / self.config.sample_rate as f32;
            let value = (f32::sin(2.0 * core::f32::consts::PI * frequency as f32 * t) * 0.3 * i16::MAX as f32) as i16;
            buffer.push((value & 0xFF) as u8);
            buffer.push((value >> 8) as u8);
        }

        buffer
    }
}

pub enum SpeakerError {
    I2SError(esp_idf_svc::sys::EspError),
    PeripheralError,
    NotInitialized,
    QueueError(String),
}

impl std::fmt::Debug for SpeakerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
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