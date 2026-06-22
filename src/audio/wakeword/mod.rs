use std::{
    io::Read,
    sync::{
        atomic::{AtomicBool, AtomicU8, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};

use log::{error, info};
use rustpotter::{Rustpotter, RustpotterConfig, RustpotterDetection, SampleFormat};

use crate::audio::MicrophoneService;

pub enum WakeWordError {
    RustpotterError,
    NotInitialized,
}

impl std::fmt::Debug for WakeWordError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WakeWordError::RustpotterError => write!(f, "Rustpotter错误"),
            WakeWordError::NotInitialized => write!(f, "未初始化"),
        }
    }
}

impl std::fmt::Display for WakeWordError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WakeWordError::RustpotterError => write!(f, "Rustpotter错误"),
            WakeWordError::NotInitialized => write!(f, "未初始化"),
        }
    }
}

impl std::error::Error for WakeWordError {}

pub struct WakeWordConfig {
    pub wakeword: String,
    pub model_path: String,
    // 采样率，默认 16000
    pub sample_rate_hz: usize,
    // 通道数，默认 1
    pub channels: u16,
    pub detector_min_scores: usize,
    pub detector_avg_threshold: f32,
    pub detector_threshold: f32,
}

impl Default for WakeWordConfig {
    fn default() -> Self {
        Self {
            wakeword: "nihaoxiaoliang".to_string(),
            model_path: "/storage/nihaoxiaoliang.rpw".to_string(),
            sample_rate_hz: 16000,
            channels: 1,
            detector_min_scores: 3,
            detector_avg_threshold: 0.1,
            detector_threshold: 0.15,
        }
    }
}

enum WakeWordState {
    Started = 0,    // 服务已启动
    WaitWaking = 1, // 监听唤醒词并等待唤醒
    Working = 2,    // 已经唤醒，进入工作状态
    Stopped = 3,    // 服务已停止
}

impl From<WakeWordState> for u8 {
    fn from(state: WakeWordState) -> Self {
        state as u8
    }
}

impl From<u8> for WakeWordState {
    fn from(state: u8) -> Self {
        match state {
            0 => WakeWordState::Started,
            1 => WakeWordState::WaitWaking,
            2 => WakeWordState::Working,
            3 => WakeWordState::Stopped,
            _ => panic!("未知状态: {}", state),
        }
    }
}

pub struct WakeWordService {
    config: RustpotterConfig,
    state: Arc<AtomicU8>,
    worker_thread: Option<thread::JoinHandle<()>>,
}

impl WakeWordService {
    pub fn new<F>(
        wakeword_config: WakeWordConfig,
        microphone_service: Arc<MicrophoneService>,
        on_detect: F,
    ) -> Result<WakeWordService, WakeWordError>
    where
        F: FnMut(&str) + Send + 'static,
    {
        info!("========================================");
        info!("🔊 唤醒词服务初始化...");
        info!("========================================");
        info!(
            "唤醒词配置: 词={}, 模型路径={}",
            wakeword_config.wakeword, wakeword_config.model_path
        );
        info!(
            "音频配置: 采样率={}Hz, 通道数={}",
            wakeword_config.sample_rate_hz, wakeword_config.channels
        );
        info!(
            "检测配置: 最小分数={}, 平均阈值={}, 阈值={}",
            wakeword_config.detector_min_scores,
            wakeword_config.detector_avg_threshold,
            wakeword_config.detector_threshold
        );

        let config: RustpotterConfig = rustpotter_config(&wakeword_config);
        info!("正在创建 Rustpotter 实例...");
        let mut rustpotter = Rustpotter::new(&config).map_err(|e| {
            error!("❌ 创建 Rustpotter 实例失败: {:?}", e);
            WakeWordError::RustpotterError
        })?;
        info!("✅ Rustpotter 实例创建成功");

        info!("正在加载唤醒词模型: {}", wakeword_config.model_path);
        rustpotter
            .add_wakeword_from_file(
                wakeword_config.wakeword.as_str(),
                wakeword_config.model_path.as_str(),
            )
            .map_err(|e| {
                error!("❌ 加载唤醒词模型失败: {:?}", e);
                error!("模型路径: {}", wakeword_config.model_path);
                WakeWordError::RustpotterError
            })?;
        info!("✅ 唤醒词模型加载成功: {}", wakeword_config.wakeword);

        // 启动工作线程

        let state = Arc::new(AtomicU8::new(WakeWordState::Started as u8));
        let state_clone = state.clone();
        Self::run_work(state_clone, rustpotter, on_detect, microphone_service);

        Ok(Self {
            config,
            state,
            worker_thread: None,
        })
    }

    fn run_work<F>(
        state: Arc<AtomicU8>,
        mut rustpotter: Rustpotter,
        mut on_detect: F,
        microphone_service: Arc<MicrophoneService>,
    ) where
        F: FnMut(&str) + Send + 'static,
    {
        let bytes_per_sample = rustpotter.get_bytes_per_frame();

        info!("唤醒词检测线程配置: 每帧字节数={}", bytes_per_sample);

        info!("唤醒词检测线程已启动");

        let mut wake_word_result = None;
        let mut detections = 0;
        loop {
            let current_state = state.load(Ordering::Relaxed);
            let current_state = current_state.into();

            match current_state {
                WakeWordState::Started => {
                    info!("唤醒词检测线程已启动");
                }
                WakeWordState::WaitWaking => {
                    info!("唤醒词检测线程正在监听唤醒词");
                    let mut audio_reader = microphone_service.open_reader().unwrap();
                    // buffer_size = bytes_per_sample
                    let mut buffer = vec![0u8; bytes_per_sample];
                    let mut frame = vec![0u8; bytes_per_sample];
                    let mut data_size: usize = 0;

                    'outer: loop {
                        // 校验是否stop
                        if state.load(Ordering::Relaxed) == WakeWordState::Stopped as u8 {
                            break;
                        }

                        let mut read_bytes = audio_reader.read(&mut buffer).unwrap();
                        while read_bytes > 0 {
                            let copy_bytes = read_bytes.min(bytes_per_sample - data_size);
                            frame[data_size..].copy_from_slice(&buffer[..copy_bytes]);
                            data_size += copy_bytes;
                            read_bytes -= copy_bytes;
                            if data_size == bytes_per_sample {
                                let result = rustpotter.process_bytes(&frame);
                                if let Some(detection) = result {
                                    detections += 1;
                                    info!(
                                        "🎉 检测到唤醒词: {} (第 {} 次)",
                                        detection.name, detections
                                    );
                                    // 跳出循环，进入Working状态
                                    state.store(WakeWordState::Working as u8, Ordering::Relaxed);
                                    wake_word_result = Some(detection);
                                    break 'outer; // 跳出循环，进入Working状态
                                }
                                data_size = 0;
                            }
                        }
                    }
                }
                WakeWordState::Working => {
                    info!("唤醒词检测线程正在工作");
                    if let Some(detection) = wake_word_result.take() {
                        on_detect(&detection.name);
                    }
                    //进入WaitWaking状态
                    state.store(WakeWordState::WaitWaking as u8, Ordering::Relaxed);
                }
                WakeWordState::Stopped => {
                    info!("唤醒词检测线程已停止");
                    break;
                }
            }
        }
        info!("唤醒词检测线程退出");
    }

    pub fn stop(&mut self) -> Result<(), WakeWordError> {
        info!("开始停止唤醒词检测服务...");
        self.state
            .store(WakeWordState::Stopped as u8, Ordering::Relaxed);

        if let Some(thread) = self.worker_thread.take() {
            info!("等待唤醒词检测线程退出...");
            if let Err(e) = thread.join() {
                error!("❌ 线程 join 失败: {:?}", e);
            } else {
                info!("✅ 唤醒词检测线程已退出");
            }
        }

        info!("✅ 唤醒词检测服务已停止");
        Ok(())
    }
}

fn rustpotter_config(wakeword_config: &WakeWordConfig) -> RustpotterConfig {
    let mut config: RustpotterConfig = RustpotterConfig::default();
    config.fmt.sample_rate = wakeword_config.sample_rate_hz;
    config.fmt.channels = wakeword_config.channels;
    config.fmt.sample_format = SampleFormat::I16;

    config.detector.min_scores = wakeword_config.detector_min_scores;
    config.detector.avg_threshold = wakeword_config.detector_avg_threshold;
    config.detector.threshold = wakeword_config.detector_threshold;
    config.detector.eager = true;

    config.filters.gain_normalizer.enabled = true;
    config.filters.gain_normalizer.max_gain = 2.0;

    config
}
