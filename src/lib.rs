//! # Feetech Servo SDK
//!
//! 这是一个高性能、基于异步 Rust (Tokio) 的飞特 (Feetech) 串口总线舵机 SDK。
//! 专门设计用于控制 STS、SMS 等型号的串行总线舵机。
//!
//! 本 SDK 旨在为机器人开发者提供一个极其稳健、类型安全且易于使用的底层控制库。它不仅支持真实的硬件通信，
//! 还内置了强大的模拟 (Mock) 后端，方便在没有硬件的情况下进行算法开发与逻辑测试。
//!
//! ## 核心特点
//!
//! - **🚀 异步驱动**: 基于 `tokio` 和 `tokio-serial` 构建，完美适配现代异步机器人控制系统。
//! - **🛡️ 鲁棒通讯**: 内置 RX 缓冲区自动排空、发送超时重试、校验和自动验证，有效抵御电气噪声干扰。
//! - **🧩 统一抽象**: 通过 `MotorBus` Trait 统合了真实总线 (`FeetechBus`) 与模拟总线 (`MockBus`)，方便进行依赖注入。
//! - **⚡ 批量操作**: 深度优化了同步写入 (`sync_write_goals`) 和读取操作，最大化总线带宽利用率。
//! - **📐 物理单位**: 内部逻辑优先采用标准国际单位（如：弧度），同时保留 `RawEffort` 接口满足底层调试需求。
//!
//! ## 硬件要求
//!
//! 飞特总线舵机通常运行在 **1,000,000 bps (1MB)** 的波特率下。
//! 请确保您的 USB 转 TTL / RS485 模块支持此速率，并且舵机已正确供电（通常为 6V~12V）。
//!
//! ## 基本用法
//!
//! ```no_run
//! use feetech_servo_sdk::{FeetechBus, MotorBus, ControlOp};
//! use std::f32::consts::PI;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // 1. 初始化串口总线
//!     let mut bus = FeetechBus::new("/dev/ttyUSB0", 1_000_000)?;
//!     
//!     // 2. 使能舵机力矩 (支持批量操作)
//!     let target_ids = vec![1, 2, 3];
//!     bus.enable_torque(&target_ids).await?;
//!     
//!     // 3. 读取位置 (弧度)
//!     let pos = bus.read_position(1).await?;
//!     println!("舵机 1 当前位置: {:.2} rad", pos);
//!     
//!     // 4. 发送控制指令 (位置控制)
//!     bus.write_goal(1, ControlOp::Position(PI)).await?; // 移动到 180 度
//!     
//!     Ok(())
//! }
//! ```
//!
//! ## 高级批量操作
//!
//! 对于多自由度机械臂，使用 `sync_write_goals` 可以有效降低指令延迟：
//!
//! ```no_run
//! # use feetech_servo_sdk::{FeetechBus, MotorBus, ControlOp};
//! # async fn doc_example(mut bus: FeetechBus) -> anyhow::Result<()> {
//! let commands = vec![
//!     (1, ControlOp::Position(0.0)),
//!     (2, ControlOp::Position(1.57)),
//!     (3, ControlOp::RawEffort(2048)), // 直接操作原始刻度
//! ];
//! bus.sync_write_goals(&commands).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## 模拟测试 (Testing & Simulation)
//!
//! 在开发阶段，您可以启用 `mock` feature 来使用 `MockBus`：
//!
//! ```toml
//! [dependencies]
//! feetech-servo-sdk = { version = "0.2.0", features = ["mock"] }
//! ```
//!
//! ```no_run
//! # #[cfg(feature = "mock")]
//! # {
//! use feetech_servo_sdk::MockBus;
//! let mut bus = MockBus::new(&[1, 2, 3]);
//! // 接下来的代码与使用真实 FeetechBus 完全一致
//! # }
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
pub use driver::FeetechController;
pub use error::ServoError;

#[cfg(feature = "tokio-serial-impl")]
pub use driver::FeetechBus;

#[cfg(feature = "mock")]
pub use mock::MockBus;
