use std::io::Read;
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
    Undefined(String),
    Stopped,
}

impl std::fmt::Display for MicrophoneError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MicrophoneError::I2SError(e) => write!(f, "I2S错误: {}", e),
            MicrophoneError::PeripheralError => write!(f, "外设错误"),
            MicrophoneError::NotInitialized => write!(f, "未初始化"),
            MicrophoneError::Undefined(msg) => write!(f, "未定义错误: {}", msg),
            MicrophoneError::Stopped => write!(f, "已停止"),
        }
    }
}

impl std::error::Error for MicrophoneError {}

impl From<esp_idf_svc::sys::EspError> for MicrophoneError {
    fn from(err: esp_idf_svc::sys::EspError) -> Self {
        MicrophoneError::I2SError(err)
    }
}

pub struct MicrophoneServiceConfig {
    pub channel_capacity: usize,
    pub read_timeout_ms: u32,
    pub read_buffer_size: usize,
    pub microphone_config: MicrophoneConfig,
}

impl Default for MicrophoneServiceConfig {
    fn default() -> Self {
        Self {
            channel_capacity: 10,
            read_timeout_ms: 100,
            read_buffer_size: 1024,
            microphone_config: MicrophoneConfig::default(),
        }
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

type DataConsumer = Box<dyn Fn(Vec<u8>) + Send>;

pub struct MicrophoneService {
    service_config: Arc<MicrophoneServiceConfig>,
    stop_signal: Arc<AtomicBool>,
    worker_thread: Option<thread::JoinHandle<()>>,
    data_consumers: Arc<Mutex<Vec<DataConsumer>>>,
    // i2s: Option<I2sDriver<'static, I2sRx>>,
}

impl MicrophoneService {
    pub fn new(service_config: MicrophoneServiceConfig) -> Result<Self, MicrophoneError> {
        info!("========================================");
        info!("🎤 麦克风程序启动...");
        info!("========================================");
        let config = &service_config.microphone_config;
        info!(
            "麦克风配置: 采样率={}Hz, 缓冲区大小={}, 超时={}ms",
            config.sample_rate, config.buffer_size, config.read_timeout_ms
        );
        info!(
            "I2S引脚配置: SCK={}, SD={}, WS={}",
            config.sck_pin, config.sd_pin, config.ws_pin
        );

        let peripherals = Peripherals::take().map_err(|e| {
            error!("❌ 获取外设失败: {:?}", e);
            MicrophoneError::PeripheralError
        })?;

        // 配置 I2S 音频输入（麦克风）- 标准模式
        info!("初始化 I2S 音频输入...");
        let i2s_config = StdConfig::philips(config.sample_rate, DataBitWidth::Bits16);
        info!(
            "I2S配置: 标准模式, 采样率={}Hz, 位宽=16位",
            config.sample_rate
        );

        info!("正在创建 I2sDriver 实例...");
        let i2s = I2sDriver::<I2sRx>::new_std_rx(
            peripherals.i2s0,
            &i2s_config,
            peripherals.pins.gpio5,                // SCK (Serial Clock)
            peripherals.pins.gpio6,                // SD (Serial Data)
            None::<esp_idf_svc::hal::gpio::Gpio0>, // MCLK（不使用）
            peripherals.pins.gpio4,                // WS (Word Select)
        )
        .map_err(|e| {
            error!("❌ 创建 I2sDriver 失败: {:?}", e);
            MicrophoneError::from(e)
        })?;
        info!("✅ I2S 音频输入初始化成功");

        let channel_capacity = service_config.channel_capacity;
        info!("创建共享发送器，通道容量: {}", channel_capacity);

        // 开启worker线程
        let data_consumers = Arc::new(Mutex::new(Vec::new()));
        let data_consumers_clone = data_consumers.clone();
        let stop_signal = Arc::new(AtomicBool::new(false));
        let stop_signal_clone = stop_signal.clone();
        let service_config = Arc::new(service_config);
        let service_config_clone = service_config.clone();
        let worker_thread = thread::spawn(move || {
            Self::run_work(
                stop_signal_clone,
                service_config_clone,
                i2s,
                data_consumers_clone,
            )
        });

        Ok(Self {
            service_config,
            worker_thread: Some(worker_thread),
            stop_signal,
            data_consumers,
        })
    }

    pub fn is_stoped(&self) -> bool {
        self.stop_signal.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn check_stoped(&self) -> Result<(), MicrophoneError> {
        if self.is_stoped() {
            return Err(MicrophoneError::Stopped);
        }
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

    fn register_data_consumer(&self, consumer: DataConsumer) -> Result<usize, MicrophoneError> {
        let mut audio_consumers = self
            .data_consumers
            .lock()
            .map_err(|e| MicrophoneError::Undefined(e.to_string()))?;
        audio_consumers.push(consumer);
        Ok(audio_consumers.len() - 1)
    }
    fn unregister_data_consumer(&self, index: usize) -> Result<DataConsumer, MicrophoneError> {
        do_unregister_data_consumer(self.data_consumers.clone(), index)
    }

    pub fn open_reader(&self) -> Result<MicrophoneReader, MicrophoneError> {
        self.check_stoped()?;
        info!("开始打开麦克风并注册读取器...");
        let (tx, rx) = mpsc::channel::<Vec<u8>>();
        let data_consumer = Box::new(move |bytes: Vec<u8>| {
            if let Err(e) = tx.send(bytes) {
                eprintln!("[麦克风] 发送音频数据失败: {}", e);
            }
        });
        let consumer_index = self.register_data_consumer(data_consumer)?;
        let data_consumers_clone = self.data_consumers.clone();
        let on_close = Box::new(move || {
            let _ = do_unregister_data_consumer(data_consumers_clone, consumer_index);
        });
        let reader = MicrophoneReader::new(on_close, rx);
        Ok(reader)
    }

    fn run_work(
        stop_signal: Arc<AtomicBool>,
        service_config: Arc<MicrophoneServiceConfig>,
        mut i2s: I2sDriver<'static, I2sRx>,
        data_consumers: Arc<Mutex<Vec<DataConsumer>>>,
    ) {
        // 开启线程，持续读取音频数据
        info!("正在启动音频数据处理线程...");
        let mut buffer = vec![0u8; service_config.read_buffer_size];
        let read_timeout_ms = service_config.read_timeout_ms;
        info!(
            "音频处理线程配置: 缓冲区大小={}, 读取超时={}ms",
            service_config.read_buffer_size, read_timeout_ms
        );
        info!("音频数据处理线程已启动");

        loop {
            if stop_signal.load(std::sync::atomic::Ordering::Relaxed) {
                info!("音频数据处理线程收到停止信号，退出循环");
                break;
            }

            let read_result = i2s.read(buffer.as_mut(), read_timeout_ms);
            match read_result {
                Ok(count) if count > 0 => {
                    let audio_consumers = data_consumers.lock().unwrap();
                    for consumer in audio_consumers.iter() {
                        consumer(Vec::from(&buffer[..count]));
                    }
                }
                Ok(_) => {
                    // 暂停处理
                    thread::sleep(Duration::from_millis(20));
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
        // 关闭I2S接收通道
        info!("正在禁用 I2S 接收通道...");
        let _ = i2s.rx_disable();
        stop_signal.store(true, std::sync::atomic::Ordering::Relaxed);
        info!("音频数据处理线程已退出");
    }
}

impl Drop for MicrophoneService {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

fn do_unregister_data_consumer(
    audio_consumers: Arc<Mutex<Vec<DataConsumer>>>,
    index: usize,
) -> Result<DataConsumer, MicrophoneError> {
    let mut audio_consumers = audio_consumers
        .lock()
        .map_err(|e| MicrophoneError::Undefined(e.to_string()))?;
    let consumer = audio_consumers.remove(index);
    Ok(consumer)
}

const EMPTY_BUFFER: Vec<u8> = Vec::new();

pub struct MicrophoneReader {
    on_close: Option<Box<dyn FnOnce() + Send>>,
    rx: mpsc::Receiver<Vec<u8>>,
    current_buffer: Vec<u8>,
    cur_index: usize,
    timeout: std::time::Duration,
}

impl MicrophoneReader {
    pub fn new(on_close: Box<dyn FnOnce() + Send>, rx: mpsc::Receiver<Vec<u8>>) -> Self {
        Self {
            on_close: Some(on_close),
            rx,
            current_buffer: EMPTY_BUFFER,
            cur_index: 0,
            timeout: std::time::Duration::from_millis(20),
        }
    }

    pub fn set_timeout(&mut self, timeout: std::time::Duration) {
        self.timeout = timeout;
    }

    pub fn close(&mut self) {
        let on_close = self.on_close.take();
        on_close.map(|f| f());
        self.current_buffer = EMPTY_BUFFER;
        self.cur_index = 0;
    }
}

impl Read for MicrophoneReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let buf_len = buf.len();
        let mut read_len = 0;
        for i in 0..buf_len {
            if self.cur_index >= self.current_buffer.len() {
                // 缓冲区空了，等待新数据
                if let Ok(new_buffer) = self.rx.recv_timeout(self.timeout) {
                    self.current_buffer = new_buffer;
                } else {
                    self.current_buffer = EMPTY_BUFFER;
                }
                self.cur_index = 0;
            }
            if self.cur_index >= self.current_buffer.len() {
                break;
            }
            buf[i] = self.current_buffer[self.cur_index];
            self.cur_index += 1;
            read_len += 1;
        }
        Ok(read_len)
    }
}

impl Drop for MicrophoneReader {
    fn drop(&mut self) {
        self.close();
    }
}
