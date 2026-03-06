//! # Feetech Servo SDK
//!
//! 这是一个高性能、基于异步 Rust (Tokio) 的飞特 (Feetech) 串口总线舵机 SDK。
//! 专门设计用于控制 STS、SMS 等型号的串行总线舵机。
//!
//! 本 SDK 提供了稳定且高效的设备通信层，并内置了对串行总线通信错误的鲁棒性处理（如：超时重传、缓冲区清理等）。
//!
//! ## 核心特点
//!
//! - **异步优先**: 基于 `tokio` 和 `tokio-serial` 构建，非阻塞控制多路舵机。
//! - **强类型控制**: 使用 `ControlOp` 枚举明确控制意图（位置、速度、扭矩等）。
//! - **批量操作**: 提供高效的同步写入 (`sync_write_goals`) 和位置居中 (`sync_set_middle_positions`) 等批量指令。
//! - **模拟测试**: 提供独立的 `MockBus` 后端进行不依赖硬件的快速迭代验证（需开启 `mock` feature）。
//!
//! ## 基本用法
//!
//! ```no_run
//! use feetech_servo_sdk::{FeetechBus, MotorBus, ControlOp};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // 初始化串口
//!     let mut bus = FeetechBus::new("/dev/ttyUSB0", 1_000_000)?;
//!     
//!     // 开启力矩
//!     bus.enable_torque(&[1, 2]).await?;
//!     
//!     // 读取位置 (弧度)
//!     let pos = bus.read_position(1).await?;
//!     println!("Current Position: {} rad", pos);
//!     
//!     // 写入目标位置 (弧度)
//!     let target_rad = 3.14; // 180度
//!     bus.write_goal(1, ControlOp::Position(target_rad)).await?;
//!     
//!     Ok(())
//! }
//! ```

pub mod bus;
pub mod driver;
pub mod error;
pub(crate) mod protocol;

// 添加 Mock 模块，并使其公开
#[cfg(feature = "mock")] // 可选：只在开启 mock feature 时编译
pub mod mock;

// Re-export common types
pub use bus::{ControlOp, MotorBus};
pub use driver::FeetechBus;
pub use error::ServoError;

#[cfg(feature = "mock")]
pub use mock::MockBus; // 可选导出
