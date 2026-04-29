use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use esp_idf_svc::hal::i2s::config::{DataBitWidth, StdConfig};
use esp_idf_svc::hal::i2s::{I2sDriver, I2sRx};
use esp_idf_svc::hal::peripherals::Peripherals;
use log::{error, info};

#[derive(Debug)]
pub enum MicrophoneError {
    I2SError(esp_idf_svc::sys::EspError),
    PeripheralError,
    NotInitialized,
}

impl std::fmt::Display for MicrophoneError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MicrophoneError::I2SError(e) => write!(f, "I2S错误: {}", e),
            MicrophoneError::PeripheralError => write!(f, "外设错误"),
            MicrophoneError::NotInitialized => write!(f, "未初始化"),
        }
    }
}

impl std::error::Error for MicrophoneError {}

impl From<esp_idf_svc::sys::EspError> for MicrophoneError {
    fn from(err: esp_idf_svc::sys::EspError) -> Self {
        MicrophoneError::I2SError(err)
    }
}

pub struct MicrophoneConfig {
    pub sample_rate: u32,
    pub buffer_size: usize,
    pub read_timeout_ms: u32,
    pub sck_pin: i32,
    pub sd_pin: i32,
    pub ws_pin: i32,
}

impl Default for MicrophoneConfig {
    fn default() -> Self {
        Self {
            sample_rate: 16000,
            buffer_size: 32,
            read_timeout_ms: 100,
            sck_pin: 5,
            sd_pin: 6,
            ws_pin: 4,
        }
    }
}

pub struct AudioDataConsumer {
    receiver: mpsc::Receiver<Vec<u8>>,
}

impl AudioDataConsumer {
    pub fn read(&mut self) -> Option<Vec<u8>> {
        self.receiver.recv().ok()
    }

    pub fn try_read(&mut self) -> Option<Vec<u8>> {
        self.receiver.try_recv().ok()
    }

    pub fn read_timeout(&mut self, timeout: Duration) -> Option<Vec<u8>> {
        self.receiver.recv_timeout(timeout).ok()
    }
}

struct SharedSenders {
    senders: Mutex<Vec<mpsc::SyncSender<Vec<u8>>>>,
}

impl SharedSenders {
    fn new() -> Self {
        Self {
            senders: Mutex::new(Vec::new()),
        }
    }

    fn add_sender(&self, sender: mpsc::SyncSender<Vec<u8>>) {
        let mut senders = self.senders.lock().unwrap();
        senders.push(sender);
    }

    fn broadcast(&self, data: Vec<u8>) {
        let senders = self.senders.lock().unwrap();
        for sender in senders.iter() {
            // 不会阻塞，立即返回
            let _ = sender.try_send(data.clone());
        }
    }
}

pub struct MicrophoneService {
    config: MicrophoneConfig,
    stop_signal: Arc<AtomicBool>,
    i2s: Option<I2sDriver<'static, I2sRx>>,
    worker_thread: Option<thread::JoinHandle<()>>,
    shared_senders: Arc<SharedSenders>,
    channel_capacity: usize,
}

impl MicrophoneService {
    pub fn new(config: MicrophoneConfig) -> Result<Self, MicrophoneError> {
        info!("========================================");
        info!("🎤 麦克风程序启动...");
        info!("========================================");
        info!("麦克风配置: 采样率={}Hz, 缓冲区大小={}, 超时={}ms", 
              config.sample_rate, config.buffer_size, config.read_timeout_ms);
        info!("I2S引脚配置: SCK={}, SD={}, WS={}", 
              config.sck_pin, config.sd_pin, config.ws_pin);

        let peripherals = Peripherals::take()
            .map_err(|e| {
                error!("❌ 获取外设失败: {:?}", e);
                MicrophoneError::PeripheralError
            })?;
        
        // 配置 I2S 音频输入（麦克风）- 标准模式
        info!("初始化 I2S 音频输入...");
        let i2s_config = StdConfig::philips(config.sample_rate, DataBitWidth::Bits16);
        info!("I2S配置: 标准模式, 采样率={}Hz, 位宽=16位", config.sample_rate);
        
        info!("正在创建 I2sDriver 实例...");
        let i2s = I2sDriver::<I2sRx>::new_std_rx(
            peripherals.i2s0,
            &i2s_config,
            peripherals.pins.gpio5,                // SCK (Serial Clock)
            peripherals.pins.gpio6,                // SD (Serial Data)
            None::<esp_idf_svc::hal::gpio::Gpio0>, // MCLK（不使用）
            peripherals.pins.gpio4,                // WS (Word Select)
        ).map_err(|e| {
            error!("❌ 创建 I2sDriver 失败: {:?}", e);
            MicrophoneError::from(e)
        })?;

        info!("✅ I2S 音频输入初始化成功");

        let channel_capacity = 10; // 每个通道最多缓存10个音频包，减少内存占用
        info!("创建共享发送器，通道容量: {}", channel_capacity);

        Ok(Self {
            i2s: Some(i2s),
            config,
            worker_thread: None,
            stop_signal: Arc::new(AtomicBool::new(false)),
            shared_senders: Arc::new(SharedSenders::new()),
            channel_capacity,
        })
    }

    pub fn start(&mut self) -> Result<(), MicrophoneError> {
        info!("开始启动麦克风服务...");
        let mut i2s = self.i2s.take().ok_or_else(|| {
            error!("❌ 麦克风服务未初始化");
            MicrophoneError::NotInitialized
        })?;
        
        info!("正在启用 I2S 接收通道...");
        match i2s.rx_enable() {
            Ok(()) => info!("✅ I2S 接收通道启用成功"),
            Err(e) => {
                error!("❌ 启用 I2S 接收通道失败: {:?}", e);
                self.i2s = Some(i2s); // 保存回实例
                return Err(e.into());
            }
        }
        
        info!("正在启动音频处理线程...");
        self.start_processing(i2s)?;
        info!("✅ 麦克风服务启动成功");
        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), MicrophoneError> {
        info!("开始停止麦克风服务...");
        self.stop_signal
            .store(true, std::sync::atomic::Ordering::Relaxed);
        
        if let Some(thread) = self.worker_thread.take() {
            info!("等待音频处理线程退出...");
            if let Err(e) = thread.join() {
                error!("❌ 线程 join 失败: {:?}", e);
            } else {
                info!("✅ 音频处理线程已退出");
            }
        }
        
        info!("✅ 麦克风服务已停止");
        Ok(())
    }

    pub fn gen_audio_consumer(&mut self) -> Result<AudioDataConsumer, MicrophoneError> {
        self.gen_audio_consumer_with_capacity(self.channel_capacity)
    }
    // 创建一个音频数据的消费者
    pub fn gen_audio_consumer_with_capacity(
        &mut self,
        channel_capacity: usize,
    ) -> Result<AudioDataConsumer, MicrophoneError> {
        info!("创建音频数据消费者，通道容量: {}", channel_capacity);
        let (sender, receiver) = mpsc::sync_channel(channel_capacity);
        self.shared_senders.add_sender(sender);
        info!("✅ 音频数据消费者创建成功");
        Ok(AudioDataConsumer { receiver })
    }

    fn start_processing(
        &mut self,
        mut i2s: I2sDriver<'static, I2sRx>,
    ) -> Result<(), MicrophoneError> {
        // 开启线程，持续读取音频数据
        info!("正在启动音频数据处理线程...");
        let mut buffer = vec![0u8; self.config.buffer_size * 2];
        let read_timeout_ms = self.config.read_timeout_ms;
        let stop_signal = self.stop_signal.clone();
        let shared_senders = self.shared_senders.clone();
        let buffer_size = self.config.buffer_size;
        
        info!("音频处理线程配置: 缓冲区大小={}, 读取超时={}ms", buffer_size, read_timeout_ms);
        
        let thread = thread::spawn(move || {
            info!("音频数据处理线程已启动");
            let mut read_count = 0;
            let mut broadcast_count = 0;
            
            loop {
                if stop_signal.load(std::sync::atomic::Ordering::Relaxed) {
                    info!("音频数据处理线程收到停止信号，退出循环");
                    break;
                }
                
                let read_result = i2s.read(buffer.as_mut(), read_timeout_ms);
                match read_result {
                    Ok(count) if count > 0 => {
                        read_count += 1;
                        if read_count % 1000 == 0 {
                            info!("已读取 {} 次音频数据", read_count);
                        }
                        // 生产者发送音频数据
                        let read_bytes = Vec::from(&buffer[..count]);
                        shared_senders.broadcast(read_bytes);
                        broadcast_count += 1;
                        if broadcast_count % 1000 == 0 {
                            info!("已广播 {} 次音频数据", broadcast_count);
                        }
                    }
                    Ok(_) => {
                        // 暂停处理
                        thread::sleep(Duration::from_millis(100));
                    }
                    Err(e) => {
                        // 判断是否超时异常, 否则打印错误，并跳出循环
                        if e.code() != esp_idf_svc::sys::ESP_ERR_TIMEOUT {
                            error!("❌ 读取音频数据失败: {:?}, 错误码: {}", e, e.code());
                            break;
                        }
                    }
                }
            }

            info!("音频数据处理线程统计: 读取 {} 次, 广播 {} 次", read_count, broadcast_count);
            
            // 关闭I2S接收通道
            info!("正在禁用 I2S 接收通道...");
            match i2s.rx_disable() {
                Ok(()) => info!("✅ I2S 接收通道禁用成功"),
                Err(e) => {
                    error!("❌ 禁用 I2S 接收通道失败: {:?}, 错误码: {}", e, e.code());
                    error!("I2S通道状态: 可能未启用或已被禁用");
                }
            }
            
            info!("音频数据处理线程已退出");
        });
        
        self.worker_thread = Some(thread);
        info!("✅ 音频数据处理线程启动成功");
        Ok(())
    }
}
