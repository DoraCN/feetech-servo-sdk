use thiserror::Error;

#[derive(Error, Debug)]
pub enum ServoError {
    #[error("IO Error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Timeout waiting for servo {id} response")]
    Timeout { id: u8 },

    #[error("Checksum mismatch from servo {id}. Expected {expected:02X}, got {actual:02X}")]
    ChecksumMismatch { id: u8, expected: u8, actual: u8 },

    #[error("Protocol error: {0}")]
    Protocol(String),

    // 严格模式：只要硬件状态位报错，就返回此错误
    #[error("Hardware error on servo {id} (Status: {status_byte:08b}): {msg}")]
    HardwareError { id: u8, status_byte: u8, msg: String },
}

pub type Result<T> = std::result::Result<T, ServoError>;