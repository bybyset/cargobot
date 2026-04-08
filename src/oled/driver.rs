use esp_idf_svc::hal::i2c::{I2cDriver, I2cConfig};
use esp_idf_svc::hal::peripherals::Peripherals;
use log::{info, error};
use esp_idf_svc::hal::units::Hertz;

const SSD1306_ADDR: u8 = 0x3C;
const SSD1306_WIDTH: u8 = 128;
const SSD1306_HEIGHT: u8 = 64;
const I2C_RETRY_COUNT: u8 = 3;

pub struct OledDisplay {
    i2c: I2cDriver<'static>,
    buffer: [u8; (SSD1306_WIDTH as usize * SSD1306_HEIGHT as usize) / 8],
}

impl OledDisplay {
    pub fn new() -> Result<Self, esp_idf_svc::sys::EspError> {
        info!("正在初始化OLED驱动...");
        
        let peripherals = match Peripherals::take() {
            Ok(p) => {
                info!("✅ 成功获取ESP32-S3外设");
                p
            },
            Err(e) => {
                error!("❌ 无法获取ESP32-S3外设: {:?}", e);
                return Err(e);
            }
        };
        
        // 配置I2C总线 - 降低速度以提高稳定性
        info!("配置I2C总线...");
        let config = I2cConfig::default()
            .baudrate(Hertz(100_000)); // 降低到100kHz以提高稳定性
            
        info!("正在初始化I2C驱动...");
        let i2c = match I2cDriver::new(
            peripherals.i2c0,
            peripherals.pins.gpio41, // SDA (第2个参数)
            peripherals.pins.gpio42, // SCL (第3个参数)
            &config,
        ) {
            Ok(driver) => {
                info!("✅ I2C驱动初始化成功，GPIO41(SDA)/GPIO42(SCL)");
                driver
            },
            Err(e) => {
                error!("❌ I2C驱动初始化失败: {:?}", e);
                return Err(e);
            }
        };
        
        let mut display = Self {
            i2c,
            buffer: [0; (SSD1306_WIDTH as usize * SSD1306_HEIGHT as usize) / 8],
        };
        
        info!("正在扫描I2C设备...");
        match display.scan_i2c_devices() {
            Ok(found) => {
                if found {
                    info!("✅ 在0x{:02X}地址发现I2C设备", SSD1306_ADDR);
                } else {
                    error!("❌ 未在0x{:02X}地址发现I2C设备", SSD1306_ADDR);
                    info!("可能的原因:");
                    info!("  1. OLED显示屏未连接");
                    info!("  2. 接线错误（SDA/GND连接不正确）");
                    info!("  3. 电源电压不足（需要3.3V）");
                    info!("  4. 设备地址错误");
                    return Err(esp_idf_svc::sys::EspError::from(1).expect("创建ESP错误失败"));
                }
            },
            Err(e) => {
                error!("❌ I2C设备扫描失败: {:?}", e);
                return Err(e);
            }
        }
        
        info!("正在初始化OLED显示屏...");
        match display.init() {
            Ok(_) => {
                info!("✅ OLED显示屏初始化成功");
            },
            Err(e) => {
                error!("❌ OLED显示屏初始化失败: {:?}", e);
                return Err(e);
            }
        }
        
        Ok(display)
    }
    
    fn scan_i2c_devices(&mut self) -> Result<bool, esp_idf_svc::sys::EspError> {
        info!("正在扫描0x00到0x7F地址的I2C设备...");
        
        let mut found_ssd1306 = false;
        
        for addr in 1..127 {
            let result = self.i2c.write(addr, &[], 500);
            match result {
                Ok(_) => {
                    info!("✅ 发现I2C设备，地址: 0x{:02X}", addr);
                    if addr == SSD1306_ADDR {
                        found_ssd1306 = true;
                    }
                },
                Err(_) => {
                    // 忽略读取失败的地址
                }
            }
        }
        
        if !found_ssd1306 {
            error!("❌ 未发现SSD1306 (0x{:02X})设备", SSD1306_ADDR);
            error!("已发现的I2C设备列表已显示在上方");
        }
        
        Ok(found_ssd1306)
    }
    
    fn init(&mut self) -> Result<(), esp_idf_svc::sys::EspError> {
        info!("发送初始化命令序列...");
        
        let commands = [
            0xAE, // 关闭显示
            0xD5, // 设置时钟分频因子/振荡器频率
            0x80, // 推荐值
            0xA8, // 设置多路复用率
            0x3F, // 64行
            0xD3, // 设置显示偏移
            0x00, // 无偏移
            0x40, // 设置显示开始行
            0x8D, // 设置电荷泵
            0x14, // 启用电荷泵
            0x20, // 设置内存地址模式
            0x00, // 水平寻址模式
            0xA1, // 设置段重映射
            0xC8, // 设置COM扫描方向
            0xDA, // 设置COM引脚配置
            0x12, // 交错配置
            0x81, // 设置对比度
            0xCF, // 对比度值
            0xD9, // 设置预充电周期
            0xF1, // 预充电周期
            0xDB, // 设置VCOMH解除选择电平
            0x40, // 默认值
            0xA4, // 全局显示关闭
            0xA6, // 正常显示
            0xAF, // 开启显示
        ];
        
        for (index, &cmd) in commands.iter().enumerate() {
            info!("发送命令 {}/{}: 0x{:02X}", index + 1, commands.len(), cmd);
            
            for retry in 0..I2C_RETRY_COUNT {
                info!("  重试 {}/{}...", retry + 1, I2C_RETRY_COUNT);
                
                match self.send_command(cmd) {
                    Ok(_) => {
                        info!("✅ 命令 0x{:02X} 发送成功", cmd);
                        break;
                    },
                    Err(e) => {
                        error!("  ❌ 命令 0x{:02X} 发送失败 (重试 {}): {:?}", cmd, retry + 1, e);
                        
                        if retry == I2C_RETRY_COUNT - 1 {
                            error!("❌ 命令 0x{:02X} 发送失败，已重试 {} 次", cmd, I2C_RETRY_COUNT);
                            info!("故障排除建议:");
                            info!("  1. 检查OLED显示屏接线");
                            info!("  2. 确保SCL/GND连接牢固");
                            info!("  3. 验证I2C地址是否正确");
                            info!("  4. 检查电源电压（3.3V）");
                            return Err(e);
                        }
                        
                        // 等待后重试
                        Self::delay_ms(50);
                    }
                }
            }
        }
        
        info!("正在清除显示缓冲区...");
        match self.clear() {
            Ok(_) => info!("✅ 显示缓冲区清除成功"),
            Err(e) => {
                error!("❌ 显示缓冲区清除失败: {:?}", e);
                return Err(e);
            }
        }
        
        info!("正在更新显示...");
        match self.update() {
            Ok(_) => info!("✅ 显示更新成功"),
            Err(e) => {
                error!("❌ 显示更新失败: {:?}", e);
                return Err(e);
            }
        }
        
        info!("OLED初始化完成！");
        Ok(())
    }
    
    fn send_command(&mut self, cmd: u8) -> Result<(), esp_idf_svc::sys::EspError> {
        let buf = [0x00, cmd];
        
        info!("发送命令: 0x{:02X}", cmd);
        match self.i2c.write(SSD1306_ADDR, &buf, 1000) {
            Ok(_) => Ok(()),
            Err(e) => {
                error!("I2C写入命令失败: {:?}", e);
                Err(e)
            }
        }
    }
    
    fn send_data(&mut self, data: &[u8]) -> Result<(), esp_idf_svc::sys::EspError> {
        let mut buf = vec![0x40; 1 + data.len()];
        buf[1..].copy_from_slice(data);
        
        info!("发送数据，长度: {}", data.len());
        match self.i2c.write(SSD1306_ADDR, &buf, 1000) {
            Ok(_) => Ok(()),
            Err(e) => {
                error!("I2C写入数据失败: {:?}", e);
                Err(e)
            }
        }
    }
    
    pub fn clear(&mut self) -> Result<(), esp_idf_svc::sys::EspError> {
        self.buffer.fill(0);
        self.update()
    }
    
    pub fn update(&mut self) -> Result<(), esp_idf_svc::sys::EspError> {
        // 设置显示位置
        self.send_command(0x21)?; // 列地址设置
        self.send_command(0x00)?; // 列起始地址
        self.send_command(0x7F)?; // 列结束地址
        
        self.send_command(0x22)?; // 页地址设置
        self.send_command(0x00)?; // 页起始地址
        self.send_command(0x07)?; // 页结束地址（8页）
        
        // 复制缓冲区以避免借位检查错误
        let buffer = self.buffer.clone();
        self.send_data(&buffer)
    }
    
    pub fn draw_pixel(&mut self, x: u8, y: u8, value: bool) {
        if x >= SSD1306_WIDTH || y >= SSD1306_HEIGHT {
            return;
        }
        
        let index = (x + (y / 8) * SSD1306_WIDTH) as usize;
        let bit = y % 8;
        
        if value {
            self.buffer[index] |= 1 << bit;
        } else {
            self.buffer[index] &= !(1 << bit);
        }
    }
    
    pub fn draw_string(&mut self, x: u8, y: u8, text: &str) {
        // 8x12 字体的字符宽度为 8 像素，高度为 12 像素
        for (i, c) in text.chars().enumerate() {
            let font_data = Self::get_font_data(c);
            self.draw_char(x + (i as u8) * 8, y, &font_data);
        }
    }
    
    fn draw_char(&mut self, x: u8, y: u8, data: &[u8]) {
        // 8x12 字体绘制
        for (i, &byte) in data.iter().enumerate() {
            for j in 0..8 {
                let pixel = (byte >> j) & 0x01 != 0;
                self.draw_pixel(x + j, y + i as u8, pixel);
            }
        }
    }
    
    fn get_font_data(c: char) -> &'static [u8] {
        // 8x12 点阵字体（更大字体）
        match c {
            '0' => &[0x3E, 0x7F, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x7F, 0x3E],
            '1' => &[0x1C, 0x3C, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C, 0x3F],
            '2' => &[0x7E, 0x7F, 0x03, 0x03, 0x03, 0x3E, 0x7C, 0x60, 0x60, 0x60, 0x7F, 0x7F],
            '3' => &[0x3E, 0x7F, 0x63, 0x63, 0x63, 0x3E, 0x1E, 0x63, 0x63, 0x63, 0x63, 0x3E],
            '4' => &[0x03, 0x07, 0x0E, 0x1C, 0x38, 0x70, 0x60, 0x7F, 0x60, 0x60, 0x60, 0x60],
            '5' => &[0x7F, 0x7F, 0x60, 0x60, 0x60, 0x7F, 0x7E, 0x03, 0x03, 0x03, 0x03, 0x3E],
            '6' => &[0x3E, 0x7F, 0x60, 0x60, 0x60, 0x7F, 0x7F, 0x63, 0x63, 0x63, 0x63, 0x3E],
            '7' => &[0x7F, 0x7F, 0x03, 0x03, 0x03, 0x0E, 0x1C, 0x38, 0x38, 0x38, 0x38, 0x38],
            '8' => &[0x3E, 0x7F, 0x63, 0x63, 0x63, 0x3E, 0x7F, 0x63, 0x63, 0x63, 0x63, 0x3E],
            '9' => &[0x3E, 0x7F, 0x63, 0x63, 0x63, 0x3F, 0x1E, 0x03, 0x03, 0x03, 0x03, 0x1E],
            'A' => &[0x3C, 0x7E, 0x63, 0x63, 0x63, 0x7E, 0x7E, 0x7E, 0x63, 0x63, 0x63, 0x63],
            'B' => &[0x7F, 0x7F, 0x63, 0x63, 0x63, 0x7E, 0x63, 0x63, 0x63, 0x63, 0x7F, 0x7F],
            'C' => &[0x3E, 0x7F, 0x63, 0x60, 0x60, 0x60, 0x60, 0x60, 0x60, 0x63, 0x7F, 0x3E],
            'D' => &[0x7F, 0x7F, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x7F, 0x7F],
            'E' => &[0x7F, 0x7F, 0x60, 0x60, 0x60, 0x7E, 0x7E, 0x60, 0x60, 0x60, 0x60, 0x7F],
            'F' => &[0x7F, 0x7F, 0x60, 0x60, 0x60, 0x7E, 0x7E, 0x60, 0x60, 0x60, 0x60, 0x60],
            'G' => &[0x3E, 0x7F, 0x63, 0x60, 0x60, 0x60, 0x63, 0x63, 0x63, 0x63, 0x7F, 0x3E],
            'H' => &[0x63, 0x63, 0x63, 0x63, 0x63, 0x7F, 0x7F, 0x63, 0x63, 0x63, 0x63, 0x63],
            'I' => &[0x3E, 0x3E, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C, 0x3E, 0x3E],
            'J' => &[0x1F, 0x3F, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x3E],
            'K' => &[0x63, 0x63, 0x63, 0x66, 0x6C, 0x78, 0x78, 0x6C, 0x66, 0x63, 0x63, 0x63],
            'L' => &[0x60, 0x60, 0x60, 0x60, 0x60, 0x60, 0x60, 0x60, 0x60, 0x63, 0x7F, 0x7F],
            'M' => &[0x63, 0x77, 0x7F, 0x6B, 0x6B, 0x6B, 0x6B, 0x6B, 0x6B, 0x6B, 0x6B, 0x6B],
            'N' => &[0x63, 0x63, 0x6F, 0x6F, 0x6F, 0x6F, 0x6B, 0x6B, 0x6B, 0x67, 0x63, 0x63],
            'O' => &[0x3E, 0x7F, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x7F, 0x3E],
            'P' => &[0x7F, 0x7F, 0x63, 0x63, 0x63, 0x7F, 0x7F, 0x60, 0x60, 0x60, 0x60, 0x60],
            'Q' => &[0x3E, 0x7F, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x6B, 0x6B, 0x7F, 0x3F],
            'R' => &[0x7F, 0x7F, 0x63, 0x63, 0x63, 0x7F, 0x7F, 0x6C, 0x66, 0x63, 0x63, 0x63],
            'S' => &[0x3E, 0x7F, 0x60, 0x60, 0x60, 0x3E, 0x1F, 0x03, 0x03, 0x03, 0x7F, 0x3E],
            'T' => &[0x7F, 0x3E, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C],
            'U' => &[0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x7F, 0x3E],
            'V' => &[0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x36, 0x36, 0x1E, 0x1E, 0x0F, 0x0F],
            'W' => &[0x63, 0x63, 0x63, 0x63, 0x63, 0x6B, 0x6B, 0x7F, 0x7F, 0x7F, 0x6B, 0x6B],
            'X' => &[0x63, 0x63, 0x36, 0x36, 0x1E, 0x1E, 0x1E, 0x1E, 0x36, 0x36, 0x63, 0x63],
            'Y' => &[0x63, 0x63, 0x63, 0x63, 0x36, 0x36, 0x1E, 0x1E, 0x0C, 0x0C, 0x0C, 0x0C],
            'Z' => &[0x7F, 0x7F, 0x03, 0x06, 0x0C, 0x18, 0x30, 0x30, 0x60, 0x63, 0x7F, 0x7F],
            ' ' => &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
            '-' => &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00],
            '.' => &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1E],
            '_' => &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF],
            '=' => &[0x00, 0x00, 0x00, 0x00, 0xFF, 0xFF, 0x00, 0x00, 0xFF, 0xFF, 0x00, 0x00],
            '+' => &[0x00, 0x00, 0x00, 0x1E, 0x1E, 0x1E, 0x1E, 0x1E, 0x1E, 0x1E, 0x1E, 0x1E],
            '*' => &[0x00, 0x00, 0x66, 0x66, 0x66, 0xFF, 0xFF, 0xFF, 0x66, 0x66, 0x66, 0x00],
            '/' => &[0x00, 0x00, 0x00, 0x03, 0x06, 0x0C, 0x18, 0x30, 0x60, 0xC0, 0x80, 0x00],
            '\\'=> &[0x00, 0x80, 0xC0, 0x60, 0x30, 0x18, 0x0C, 0x06, 0x03, 0x00, 0x00, 0x00],
            '|' => &[0x1E, 0x1E, 0x1E, 0x1E, 0x1E, 0x1E, 0x1E, 0x1E, 0x1E, 0x1E, 0x1E, 0x1E],
            '!' => &[0x00, 0x00, 0x1E, 0x1E, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
            '?' => &[0x3E, 0x7F, 0x03, 0x03, 0x03, 0x0E, 0x1C, 0x00, 0x00, 0x00, 0x00, 0x00],
            '(' => &[0x0C, 0x1C, 0x36, 0x36, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x36, 0x1C],
            ')' => &[0x1C, 0x0C, 0x36, 0x36, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x36, 0x1C],
            '"' => &[0x33, 0x33, 0x33, 0x33, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
            '\''=> &[0x03, 0x03, 0x03, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
            '[' => &[0x1E, 0x3E, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x3E, 0x1E],
            ']' => &[0x3E, 0x1E, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C, 0x1E, 0x3E],
            '{' => &[0x0C, 0x1E, 0x33, 0x33, 0x60, 0x60, 0x60, 0x60, 0x33, 0x33, 0x1E, 0x0C],
            '}' => &[0x33, 0x1E, 0x0C, 0x0C, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x0C, 0x1E, 0x33],
            _ => &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00], // 未知字符显示空格
        }
    }
    
    pub fn draw_test_pattern(&mut self) {
        for y in 0..SSD1306_HEIGHT {
            for x in 0..SSD1306_WIDTH {
                let value = (x + y) % 16 < 8;
                self.draw_pixel(x, y, value);
            }
        }
    }
    
    pub fn test_display(&mut self) -> Result<(), esp_idf_svc::sys::EspError> {
        info!("开始OLED显示测试...");
        
        // 清除屏幕
        self.clear()?;
        
        // 绘制测试文字
        self.draw_string(0, 0, "OLED Test");
        self.draw_string(0, 16, "Hello!");
        self.draw_string(0, 32, "Testing 123");
        
        // 绘制分隔线
        for x in 0..SSD1306_WIDTH {
            self.draw_pixel(x, 48, true);
        }
        
        // 绘制简单图案
        for x in 0..64 {
            for y in 50..64 {
                let value = (x ^ y) % 8 < 4;
                self.draw_pixel(x, y, value);
            }
        }
        
        // 更新显示
        self.update()?;
        info!("✅ OLED显示测试完成");
        
        Ok(())
    }
    
    fn delay_ms(ms: u32) {
        use esp_idf_svc::hal::delay::Delay;
        Delay::new_default().delay_ms(ms);
    }
}