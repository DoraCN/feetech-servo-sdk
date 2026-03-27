use thiserror::Error;

/// 舵机操作中可能出现的错误类型
#[derive(Error, Debug)]
pub enum ServoError {
    /// 底层标准库 IO 错误
    #[error("IO Error: {0}")]
    Io(#[from] std::io::Error),

    /// 串口通信错误 (例如: 端口不存在、权限不足)
    #[cfg(feature = "tokio-serial-impl")]
    #[error("Serial Port Error: {0}")]
    Serial(#[from] tokio_serial::Error),

    /// 接收舵机响应超时 (发生在通信无响应时)
    #[error("Timeout waiting for servo {id} response")]
    Timeout {
        /// 发生超时的舵机 ID
        id: u8,
    },

    /// 通信校验和不匹配 (发生了数据损坏)
    #[error("Checksum mismatch from servo {id}. Expected {expected:02X}, got {actual:02X}")]
    ChecksumMismatch {
        /// 报错的舵机 ID
        id: u8,
        /// 期望的校验和
        expected: u8,
        /// 实际接收到的校验和
        actual: u8,
    },

    /// 协议解析错误 (例如: 格式不合法或不支持的操作)
    #[error("Protocol error: {0}")]
    Protocol(String),

    /// 舵机硬件级别的状态报错 (例如: 过热、过载)
    #[error("Hardware error on servo {id} (Status: {status_byte:08b}): {msg}")]
    HardwareError {
        /// 报错的舵机 ID
        id: u8,
        /// 原始状态字节
        status_byte: u8,
        /// 错误消息详情
        msg: String,
    },
}

/// 针对舵机控制的标准 Result 类型别名
pub type Result<T> = std::result::Result<T, ServoError>;
