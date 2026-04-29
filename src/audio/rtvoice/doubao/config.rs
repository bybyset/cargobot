pub const WS_URL: &str = "wss://openspeech.bytedance.com/api/v3/realtime/dialogue";
pub const API_RESOURCE_ID: &str = "volc.speech.dialog"; // 固定值
pub const API_APP_ID: &str = "1968235856"; //APP ID
pub const API_ACCESS_KEY: &str = "7v1w2ximy7SkcjtcH6tB_6rY_hjLDYvT"; // Access Token
pub const API_APP_KEY: &str = "PlgvMymc7f3tQnJ6"; // 固定值


pub const SAY_HELLO: &str = "你好！我是小梁，有什么可以帮助你的吗？"; // 你好！小梁

// 音频参数配置
pub const INPUT_SAMPLE_RATE: u32 = 16000;
pub const OUTPUT_SAMPLE_RATE: u32 = 24000;
pub const CHANNELS: u32 = 1;
pub const INPUT_FRAMES_PER_BUFFER: u32 = 160;
pub const OUTPUT_FRAMES_PER_BUFFER: u32 = 512;
pub const BUFFER_SECONDS: u32 = 100;

// 音频格式
pub const DEFAULT_PCM: &str = "pcm";
pub const PCM_S16LE: &str = "pcm_s16le";

// TTS配置
pub const DEFAULT_SPEAKER: &str = "zh_female_vv_jupiter_bigtts";

// 网络配置
pub const AUDIO_CHUNK_SIZE: u32 = 640; // 字节，对应20ms音频数据
pub const AUDIO_SEND_INTERVAL: u64 = 20; // 毫秒

// WAV文件配置
pub const WAV_HEADER_SIZE: u32 = 44; // WAV文件头大小

// 命令行参数默认值
pub static mut AUDIO_FILE_PATH: &str = "";
pub static mut PCM_FORMAT: &str = PCM_S16LE;

// 唤醒词
pub const WAKE_WORD: &str = "你好！小梁";


// 语音交互配置
pub const RECORD_DURATION_MS: u32 = 5000; // 录音时长（毫秒）
pub const WAKE_WORD_DETECTION_THRESHOLD: i16 = 500; // 音量阈值
pub const AUDIO_BUFFER_SIZE: usize = 1024; // 音频缓冲区大小
pub const READ_TIMEOUT_MS: u32 = 100; // 读取超时时间（毫秒）

// 错误码定义
pub const ERROR_CODE_SUCCESS: i32 = 0;
pub const ERROR_CODE_INIT_FAILURE: i32 = -1;
pub const ERROR_CODE_NETWORK_FAILURE: i32 = -2;
pub const ERROR_CODE_AUDIO_PROCESSING_FAILURE: i32 = -3;
pub const ERROR_CODE_WAKE_WORD_NOT_FOUND: i32 = -4;

// 状态码定义
pub const STATUS_IDLE: i32 = 0;
pub const STATUS_DETECTING: i32 = 1;
pub const STATUS_RECORDING: i32 = 2;
pub const STATUS_PROCESSING: i32 = 3;
pub const STATUS_PLAYING: i32 = 4;
pub const STATUS_ERROR: i32 = 5;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_constants() {
        assert_eq!(WS_URL, "wss://openspeech.bytedance.com/api/v3/realtime/dialogue");
        assert_eq!(API_APP_ID, "1968235856");
        assert_eq!(API_ACCESS_KEY, "7v1w2ximy7SkcjtcH6tB_6rY_hjLDYvT");
        assert_eq!(API_APP_KEY, "PlgvMymc7f3tQnJ6");
        
        assert_eq!(INPUT_SAMPLE_RATE, 16000);
        assert_eq!(OUTPUT_SAMPLE_RATE, 24000);
        assert_eq!(CHANNELS, 1);
        assert_eq!(INPUT_FRAMES_PER_BUFFER, 160);
        assert_eq!(OUTPUT_FRAMES_PER_BUFFER, 512);
        assert_eq!(BUFFER_SECONDS, 100);
        
        assert_eq!(DEFAULT_PCM, "pcm");
        assert_eq!(PCM_S16LE, "pcm_s16le");
        
        assert_eq!(DEFAULT_SPEAKER, "zh_female_vv_jupiter_bigtts");
        
        assert_eq!(AUDIO_CHUNK_SIZE, 640);
        assert_eq!(AUDIO_SEND_INTERVAL, 20);
        
        assert_eq!(WAV_HEADER_SIZE, 44);
        
        assert_eq!(WAKE_WORD, "你好！小梁");
        
        assert_eq!(RECORD_DURATION_MS, 5000);
        assert_eq!(WAKE_WORD_DETECTION_THRESHOLD, 500);
        assert_eq!(AUDIO_BUFFER_SIZE, 1024);
        assert_eq!(READ_TIMEOUT_MS, 100);
        
        assert_eq!(ERROR_CODE_SUCCESS, 0);
        assert_eq!(ERROR_CODE_INIT_FAILURE, -1);
        assert_eq!(ERROR_CODE_NETWORK_FAILURE, -2);
        assert_eq!(ERROR_CODE_AUDIO_PROCESSING_FAILURE, -3);
        assert_eq!(ERROR_CODE_WAKE_WORD_NOT_FOUND, -4);
        
        assert_eq!(STATUS_IDLE, 0);
        assert_eq!(STATUS_DETECTING, 1);
        assert_eq!(STATUS_RECORDING, 2);
        assert_eq!(STATUS_PROCESSING, 3);
        assert_eq!(STATUS_PLAYING, 4);
        assert_eq!(STATUS_ERROR, 5);
    }

}
