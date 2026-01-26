use thiserror::Error;

#[derive(Error, Debug)]
pub enum ServoError {
    #[error("IO Error: {0}")]
    Io(#[from] std::io::Error),

    // ✅ 新增：专门处理 tokio_serial 的错误 (即 serialport::Error)
    #[error("Serial Port Error: {0}")]
    Serial(#[from] tokio_serial::Error),

    #[error("Timeout waiting for servo {id} response")]
    Timeout { id: u8 },

    #[error("Checksum mismatch from servo {id}. Expected {expected:02X}, got {actual:02X}")]
    ChecksumMismatch { id: u8, expected: u8, actual: u8 },

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Hardware error on servo {id} (Status: {status_byte:08b}): {msg}")]
    HardwareError {
        id: u8,
        status_byte: u8,
        msg: String,
    },
}

pub type Result<T> = std::result::Result<T, ServoError>;
