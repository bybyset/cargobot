use esp_idf_svc::hal::delay::Delay;
use esp_idf_svc::hal::i2s::config::{DataBitWidth, StdConfig};
use esp_idf_svc::hal::i2s::I2sDriver;
use esp_idf_svc::hal::i2s::I2sTx;
use log::info;

const SAMPLE_RATE: u32 = 44100; // 44.1kHz 采样率
const BITS_PER_SAMPLE: DataBitWidth = DataBitWidth::Bits16;
const BUFFER_SIZE: usize = 1024; // 音频缓冲区大小

pub struct SimpleBuzzer {
    i2s: I2sDriver<'static, I2sTx>,
    buffer: Vec<u8>,
}

impl SimpleBuzzer {
    pub fn new() -> Result<Self, esp_idf_svc::sys::EspError> {
        let peripherals = esp_idf_svc::hal::peripherals::Peripherals::take().unwrap();
        
        info!("初始化 I2S 音频输出...");
        let i2s_config = StdConfig::philips(SAMPLE_RATE, BITS_PER_SAMPLE);
        
        let mut i2s = I2sDriver::<I2sTx>::new_std_tx(
            peripherals.i2s1, // 使用 I2S1 作为音频输出
            &i2s_config,
            peripherals.pins.gpio15, // BCLK (Bit Clock)
            peripherals.pins.gpio7,  // DIN (Data Input)
            None::<esp_idf_svc::hal::gpio::Gpio0>, // MCLK（不使用）
            peripherals.pins.gpio16, // LRC (Left/Right Channel)
        )?;
        
        info!("✅ I2S 音频输出初始化成功");
        
        // 启用 I2S 发送通道
        i2s.tx_enable()?;
        info!("✅ I2S 发送通道启用成功");
        
        Ok(Self {
            i2s,
            buffer: Vec::with_capacity(BUFFER_SIZE),
        })
    }
    
    /// 生成正弦波音频数据
    fn generate_sine_wave(&self, frequency: u32, duration_ms: u32) -> Vec<u8> {
        let num_samples = (SAMPLE_RATE * duration_ms / 1000) as usize;
        let mut data = Vec::with_capacity(num_samples * 2); // 16位采样
        
        for i in 0..num_samples {
            let t = i as f32 / SAMPLE_RATE as f32;
            let value = (f32::sin(2.0 * core::f32::consts::PI * frequency as f32 * t) * 0.3 * i16::MAX as f32) as i16;
            
            // 转换为小端字节序
            data.push((value & 0xFF) as u8);
            data.push((value >> 8) as u8);
        }
        
        data
    }
    
    pub fn play_tone(&mut self, frequency: u32, duration_ms: u32) -> Result<(), esp_idf_svc::sys::EspError> {
        info!("播放音频：频率={}Hz，持续时间={}ms", frequency, duration_ms);
        
        let audio_data = self.generate_sine_wave(frequency, duration_ms);
        
        // 发送音频数据
        let mut offset = 0;
        while offset < audio_data.len() {
            let remaining = audio_data.len() - offset;
            let send_size = core::cmp::min(BUFFER_SIZE, remaining);
            
            let sent = self.i2s.write(&audio_data[offset..offset + send_size], 100)?;
            offset += sent;
        }
        
        Ok(())
    }
    
    pub fn play_connected_sound(&mut self) -> Result<(), esp_idf_svc::sys::EspError> {
        info!("播放连接成功提示音");
        
        // 上升音调表示成功
        self.play_tone(400, 200)?;
        Delay::new_default().delay_ms(50);
        self.play_tone(600, 200)?;
        Delay::new_default().delay_ms(50);
        self.play_tone(800, 400)?;
        
        Ok(())
    }
    
    pub fn play_test_sound(&mut self) -> Result<(), esp_idf_svc::sys::EspError> {
        info!("播放测试音");
        
        // 高低音交替
        self.play_tone(800, 250)?;
        Delay::new_default().delay_ms(50);
        self.play_tone(1200, 250)?;
        Delay::new_default().delay_ms(50);
        self.play_tone(800, 250)?;
        Delay::new_default().delay_ms(50);
        self.play_tone(1200, 250)?;
        
        Ok(())
    }
}
