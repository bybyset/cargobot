use std::{
    sync::{atomic::AtomicBool, Arc},
    thread,
    time::Duration,
};

use log::{error, info};
use rustpotter::{Rustpotter, RustpotterConfig, SampleFormat};

use crate::audio::AudioDataConsumer;
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

pub struct WakeWordService {
    config: RustpotterConfig,
    stop_signal: Arc<AtomicBool>,
    rustpotter: Option<Rustpotter>,
    worker_thread: Option<thread::JoinHandle<()>>,
}

impl WakeWordService {
    pub fn new(wakeword_config: WakeWordConfig) -> Result<WakeWordService, WakeWordError> {
        info!("========================================");
        info!("🔊 唤醒词服务初始化...");
        info!("========================================");
        info!("唤醒词配置: 词={}, 模型路径={}", 
              wakeword_config.wakeword, wakeword_config.model_path);
        info!("音频配置: 采样率={}Hz, 通道数={}", 
              wakeword_config.sample_rate_hz, wakeword_config.channels);
        info!("检测配置: 最小分数={}, 平均阈值={}, 阈值={}", 
              wakeword_config.detector_min_scores, 
              wakeword_config.detector_avg_threshold, 
              wakeword_config.detector_threshold);

        let config: RustpotterConfig = rustpotter_config(&wakeword_config);
        info!("正在创建 Rustpotter 实例...");
        let mut rustpotter = 
            Rustpotter::new(&config).map_err(|e| {
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

        Ok(Self {
            config,
            stop_signal: Arc::new(AtomicBool::new(false)),
            rustpotter: Some(rustpotter),
            worker_thread: None,
        })
    }

    // 启动唤醒词检测线程
    pub fn start<F>(
        &mut self,
        audio_consumer: AudioDataConsumer,
        on_detect: F,
    ) -> Result<(), WakeWordError>
    where
        F: FnMut(&str) + Send + 'static,
    {
        info!("开始启动唤醒词检测服务...");
        let mut rustpotter = self
            .rustpotter
            .take()
            .ok_or_else(|| {
                error!("❌ 唤醒词服务未初始化");
                WakeWordError::NotInitialized
            })?;

        info!("正在启动唤醒词检测线程...");
        self.start_process(audio_consumer, rustpotter, on_detect)
            .map_err(|e| {
                error!("❌ 启动唤醒词检测线程失败: {:?}", e);
                WakeWordError::RustpotterError
            })?;
        info!("✅ 唤醒词检测服务启动成功");
        Ok(())
    }

    fn start_process<F>(
        &mut self,
        mut audio_consumer: AudioDataConsumer,
        mut rustpotter: Rustpotter,
        mut on_detect: F,
    ) -> Result<(), WakeWordError>
    where
        F: FnMut(&str) + Send + 'static,
    {
        let bytes_per_sample = rustpotter.get_bytes_per_frame();
        let stop_signal = self.stop_signal.clone();

        info!("唤醒词检测线程配置: 每帧字节数={}", bytes_per_sample);
        
        let worker = thread::spawn(move || {
            info!("唤醒词检测线程已启动");
            let mut frame = Vec::with_capacity(bytes_per_sample);
            let mut audio_received = 0;
            let mut frames_processed = 0;
            let mut detections = 0;

            loop {
                if stop_signal.load(std::sync::atomic::Ordering::Relaxed) {
                    info!("唤醒词检测线程收到停止信号，退出循环");
                    break;
                }
                
                match audio_consumer.read_timeout(Duration::from_millis(100)) {
                    Some(data) => {
                        audio_received += 1;
                        if audio_received % 1000 == 0 {
                            info!("已接收 {} 次音频数据", audio_received);
                        }
                        
                        let mut data_slice = &data[..];
                        // info!("收到音频数据，长度: {} 字节", data.len());

                        while !data_slice.is_empty() {
                            let needed = bytes_per_sample - frame.len();
                            let to_take = data_slice.len().min(needed);
                            
                            // info!("处理音频数据: 需要 {} 字节, 取 {} 字节", needed, to_take);
                            frame.extend_from_slice(&data_slice[..to_take]);
                            data_slice = &data_slice[to_take..];

                            if frame.len() == bytes_per_sample {
                                frames_processed += 1;
                                if frames_processed % 100 == 0 {
                                    info!("已处理 {} 帧音频数据", frames_processed);
                                }
                                
                                // info!("处理完整音频帧，长度: {} 字节", frame.len());
                                let result = rustpotter.process_bytes(&frame);
                                if let Some(detection) = result {
                                    detections += 1;
                                    info!("🎉 检测到唤醒词: {} (第 {} 次)", detection.name, detections);
                                    on_detect(&detection.name);
                                }
                                frame.clear();
                            } else {
                                // info!("累积音频数据，当前长度: {} 字节 (需要 {} 字节)", frame.len(), bytes_per_sample);
                            }
                        }
                    },
                    None => {
                        // 超时，继续循环
                    }
                }
            }
            
            info!("唤醒词检测线程统计: 接收 {} 次音频, 处理 {} 帧, 检测到 {} 次唤醒词", 
                  audio_received, frames_processed, detections);
            // stoped
            info!("唤醒词检测线程退出");
        });
        self.worker_thread = Some(worker);
        info!("✅ 唤醒词检测线程启动成功");
        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), WakeWordError> {
        info!("开始停止唤醒词检测服务...");
        self.stop_signal
            .store(true, std::sync::atomic::Ordering::Relaxed);
        
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
