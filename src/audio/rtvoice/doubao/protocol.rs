use std::io::{Read, Write, Cursor};
use std::error::Error;
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use serde_json;
use thiserror::Error;

// 消息类型定义
#[derive(Debug, PartialEq, Clone, Copy, Serialize, Deserialize)]
pub enum MsgType {
    Invalid = 0,
    FullClient = 1,
    AudioOnlyClient = 2,
    FullServer = 9,
    AudioOnlyServer = 11,
    FrontEndResultServer = 12,
    Error = 15,
}

impl MsgType {
    pub fn from_bits(bits: u8) -> Self {
        match bits & 0x0F {
            0 => MsgType::Invalid,
            1 => MsgType::FullClient,
            2 => MsgType::AudioOnlyClient,
            9 => MsgType::FullServer,
            11 => MsgType::AudioOnlyServer,
            12 => MsgType::FrontEndResultServer,
            15 => MsgType::Error,
            _ => MsgType::Invalid,
        }
    }
}

// 消息标志
pub const MSG_TYPE_FLAG_NO_SEQ: u8 = 0;
pub const MSG_TYPE_FLAG_POSITIVE_SEQ: u8 = 0b1;
pub const MSG_TYPE_FLAG_LAST_NO_SEQ: u8 = 0b10;
pub const MSG_TYPE_FLAG_NEGATIVE_SEQ: u8 = 0b11;
pub const MSG_TYPE_FLAG_WITH_EVENT: u8 = 0b100;

// 版本和头部大小
pub const VERSION_1: u8 = 0x10;
pub const HEADER_SIZE_4: u8 = 0x1;

// 序列化和压缩
pub const SERIALIZATION_RAW: u8 = 0;
pub const SERIALIZATION_JSON: u8 = 0b1 << 4;
pub const COMPRESSION_NONE: u8 = 0;

// 消息结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub msg_type: MsgType,
    pub type_flag: u8,
    pub event: u32,
    pub session_id: String,
    pub connect_id: String,
    pub sequence: i32,
    pub error_code: i64,
    pub payload: Vec<u8>,
}

impl Default for Message {
    fn default() -> Self {
        Message {
            msg_type: MsgType::Invalid,
            type_flag: MSG_TYPE_FLAG_NO_SEQ,
            event: 0,
            session_id: String::new(),
            connect_id: String::new(),
            sequence: 0,
            error_code: 0,
            payload: Vec::new(),
        }
    }
}

// 协议解析错误
#[derive(Error, Debug)]
pub enum ProtocolError {
    #[error("IO错误: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("数据长度不足")]
    InsufficientData,
    
    #[error("JSON解析错误: {0}")]
    JsonError(#[from] serde_json::Error),
    
    #[error("无效的消息类型")]
    InvalidMessageType,
    
    #[error("未知字段")]
    UnknownField,
}

// 读取i32值的辅助函数
fn read_i32(reader: &mut Cursor<&[u8]>) -> Result<i32, ProtocolError> {
    let mut buffer = [0u8; 4];
    reader.read_exact(&mut buffer)?;
    Ok(i32::from_be_bytes(buffer))
}

// 写入i32值的辅助函数
fn write_i32(writer: &mut Cursor<Vec<u8>>, value: i32) -> Result<(), ProtocolError> {
    writer.write_all(&value.to_be_bytes())?;
    Ok(())
}

// 读取字符串的辅助函数
fn read_string(reader: &mut Cursor<&[u8]>) -> Result<String, ProtocolError> {
    let length = read_i32(reader)? as usize;
    if length > 0 {
        let mut buffer = vec![0u8; length];
        reader.read_exact(&mut buffer)?;
        Ok(String::from_utf8_lossy(&buffer).to_string())
    } else {
        Ok(String::new())
    }
}

// 写入字符串的辅助函数
fn write_string(writer: &mut Cursor<Vec<u8>>, value: &str) -> Result<(), ProtocolError> {
    let bytes = value.as_bytes();
    write_i32(writer, bytes.len() as i32)?;
    writer.write_all(bytes)?;
    Ok(())
}

// 序列化消息
pub fn marshal(msg: &Message) -> Result<Vec<u8>, ProtocolError> {
    debug!("序列化消息: {:?}", msg);
    
    let mut buffer = Vec::new();
    let mut writer = Cursor::new(buffer);
    
    // 写入头部
    let version_and_header_size = VERSION_1 | HEADER_SIZE_4;
    writer.write_all(&[version_and_header_size])?;
    
    let type_and_flag = ((msg.msg_type as u8) << 4) | (msg.type_flag & 0x0F);
    writer.write_all(&[type_and_flag])?;
    
    let serialization_and_compression = SERIALIZATION_JSON | COMPRESSION_NONE;
    writer.write_all(&[serialization_and_compression])?;
    
    writer.write_all(&[0])?; // 保留字节
    
    // 写入事件ID（如果有）
    if msg.type_flag & MSG_TYPE_FLAG_WITH_EVENT != 0 {
        write_i32(&mut writer, msg.event as i32)?;
    }
    
    // 写入会话ID
    if should_write_session_id(msg) {
        write_string(&mut writer, &msg.session_id)?;
    }
    
    // 写入连接ID
    if should_write_connect_id(msg) {
        write_string(&mut writer, &msg.connect_id)?;
    }
    
    // 写入序列ID（如果有）
    if contains_sequence(msg.type_flag) {
        write_i32(&mut writer, msg.sequence)?;
    }
    
    // 写入错误码（如果是错误消息）
    if msg.msg_type == MsgType::Error {
        write_i32(&mut writer, msg.error_code as i32)?;
    }
    
    // 写入负载
    write_i32(&mut writer, msg.payload.len() as i32)?;
    writer.write_all(&msg.payload)?;
    
    Ok(writer.into_inner())
}

// 反序列化消息
pub fn unmarshal(data: &[u8]) -> Result<Message, ProtocolError> {
    debug!("反序列化消息，长度: {}", data.len());
    
    if data.len() < 4 {
        return Err(ProtocolError::InsufficientData);
    }
    
    let mut reader = Cursor::new(data);
    let mut msg = Message::default();
    
    // 读取头部
    let version_and_header_size = read_u8(&mut reader)?;
    let type_and_flag = read_u8(&mut reader)?;
    let serialization_and_compression = read_u8(&mut reader)?;
    let reserved = read_u8(&mut reader)?;
    
    // 解析消息类型和标志
    msg.msg_type = MsgType::from_bits(type_and_flag);
    msg.type_flag = type_and_flag & 0x0F;
    
    // 读取事件ID（如果有）
    if msg.type_flag & MSG_TYPE_FLAG_WITH_EVENT != 0 {
        msg.event = read_i32(&mut reader)? as u32;
    }
    
    // 读取会话ID
    if should_write_session_id(&msg) {
        msg.session_id = read_string(&mut reader)?;
    }
    
    // 读取连接ID
    if should_write_connect_id(&msg) {
        msg.connect_id = read_string(&mut reader)?;
    }
    
    // 读取序列ID（如果有）
    if contains_sequence(msg.type_flag) {
        msg.sequence = read_i32(&mut reader)?;
    }
    
    // 读取错误码（如果是错误消息）
    if msg.msg_type == MsgType::Error {
        msg.error_code = read_i32(&mut reader)? as i64;
    }
    
    // 读取负载
    let payload_length = read_i32(&mut reader)? as usize;
    if payload_length > 0 && (reader.position() as usize + payload_length) <= data.len() {
        let mut buffer = vec![0u8; payload_length];
        reader.read_exact(&mut buffer)?;
        msg.payload = buffer;
    }
    
    debug!("反序列化完成: {:?}", msg);
    Ok(msg)
}

// 读取u8的辅助函数
fn read_u8(reader: &mut Cursor<&[u8]>) -> Result<u8, ProtocolError> {
    let mut buffer = [0u8; 1];
    reader.read_exact(&mut buffer)?;
    Ok(buffer[0])
}

// 检查是否需要写入会话ID
fn should_write_session_id(msg: &Message) -> bool {
    // 在实际应用中，根据消息类型和标志位决定是否写入
    true
}

// 检查是否需要写入连接ID
fn should_write_connect_id(msg: &Message) -> bool {
    // 在实际应用中，根据消息类型和标志位决定是否写入
    true
}

// 检查是否包含序列信息
fn contains_sequence(type_flag: u8) -> bool {
    (type_flag & 0b11) == MSG_TYPE_FLAG_POSITIVE_SEQ || 
    (type_flag & 0b11) == MSG_TYPE_FLAG_NEGATIVE_SEQ
}