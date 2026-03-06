# Feetech Servo SDK (Rust)

[![Rust](https://img.shields.io/badge/rust-stable-brightgreen.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

这是一个高性能、基于异步 Rust (Tokio) 的飞特 (Feetech/Hiwonder) 串口总线舵机 SDK。专门设计用于控制 STS、SMS 等型号的串行总线舵机。

该项目的目标是提供一个**工业级**的控制接口，满足机器人高频控制环路（如 LeRobot、DORA 节点）对低延迟、并发安全和代码鲁棒性的核心需求。

## ✨ 核心特性

* **🚀 全异步架构**: 基于 `Tokio` 和 `tokio-serial`，非阻塞 I/O，完美契合高性能异步控制系统。
* **🛡️ 类型安全与总线互斥**: 利用 Rust 的所有权模型 (`&mut self`) 确保总线访问的互斥性，在编译期杜绝指令冲突。
* **📐 物理单位优先**: 接口层统一使用 **弧度 (Radians)**，内部自动处理 `Radians <-> Raw Ticks` 转换，简化运动学实现。
* **⚡ 批处理深度优化**: 支持硬件级同步写 (`SYNC_WRITE`)，大幅降低多关节同步控制的延迟。
* **🤖 鲁棒通讯机制**: 内置 RX 缓冲区自动排空、发送超时重试、校验和自动验证，有效抵御电气噪声干扰。
* **🧪 内置模拟后端 (Mock)**: 提供纯内存的 Mock 系统，支持物理运动模拟，方便在无硬件环境下通过特征注入进行开发。

## 📦 安装

在你的 `Cargo.toml` 中添加：

```toml
[dependencies]
feetech-servo-sdk = "0.2.0"
tokio = { version = "1", features = ["full"] }
anyhow = "1.0"
```

## 💻 快速开始 (Usage)

### 1. 基础读取与位置控制

以下代码演示了如何连接串口、开启磁控力矩并移动单个舵机：

```rust
use feetech_servo_sdk::{FeetechBus, MotorBus, ControlOp};
use std::f32::consts::PI;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. 初始化总线 (Path, BaudRate)
    let mut bus = FeetechBus::new("/dev/ttyUSB0", 1_000_000)?;

    // 2. 开启力矩 (Enable Torque)
    bus.enable_torque(&[1]).await?;

    // 3. 读取当前弧度
    let pos = bus.read_position(1).await?;
    println!("ID 1 当前位置: {:.4} rad", pos);

    // 4. 移动到 180 度 (PI 弧度)
    bus.write_goal(1, ControlOp::Position(PI)).await?;

    Ok(())
}
```

### 2. 高效同步控制 (多自由度机械臂)

使用 `sync_write_goals` 可以通过单个数据包同时指挥多个关节，确保动作同步：

```rust
use feetech_servo_sdk::{FeetechBus, MotorBus, ControlOp};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut bus = FeetechBus::new("/dev/ttyUSB0", 1_000_000)?;
    let ids = vec![1, 2, 3, 4, 5, 6];

    bus.enable_torque(&ids).await?;

    // 批量读取位置
    let positions = bus.sync_read_positions(&ids).await?;
    println!("关节位置序列: {:?}", positions);

    // 批量同步写入 (SYNC_WRITE)
    let targets = vec![
        (1, ControlOp::Position(0.0)),
        (2, ControlOp::Position(1.57)),
        (3, ControlOp::RawEffort(2048)), // 支持直接写入原始刻度
    ];
    bus.sync_write_goals(&targets).await?;

    Ok(())
}
```

### 3. 模拟与仿真模式 (Mock)

在开发阶段，启用 `mock` feature 可以在没有硬件连接的情况下测试您的控制代码：

```toml
feetech-servo-sdk = { version = "0.2.0", features = ["mock"] }
```

```rust
use feetech_servo_sdk::{MockBus, MotorBus, ControlOp};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 注册需要模拟的 ID 列表
    let mut bus = MockBus::new(&[1, 2, 3]);

    // 接口与真实总线完全一致
    bus.enable_torque(&[1]).await?;
    bus.write_goal(1, ControlOp::Position(1.0)).await?;
    
    let pos = bus.read_position(1).await?;
    println!("模拟器当前位置: {}", pos);

    Ok(())
}
```

## 🛠️ 命令行调试工具 (CLI)

SDK 内置了常用的调试指令，可以快速测试硬件状态。

```bash
# 扫描串口上的所有舵机
cargo run --example scan -- --port /dev/ttyUSB0

# 读取原始位置数值
cargo run --example read_raw -- --port /dev/ttyUSB0 --ids 1,2,3

# 简单的机械臂遥操作演示 (主从镜像)
cargo run --example teleop_simple_safe -- --leader-port /dev/ttyUSB0 --follower-port /dev/ttyUSB1
```

## ⚙️ 硬件连接指南

* **电源**: 飞特 STS/SMS 舵机通常需要 **6V ~ 12V** 外部供电（推荐 2S 锂电 7.4V）。
* **信号**: 串行总线通常为 **单线半双工 (TTL)**。连接 Mac/PC 时，需要使用专用的信号转接板（如飞特原厂转接板或 DIY 带有三态缓冲器的电路）。
* **波特率**: 默认出厂波特率为 **1,000,000 (1Mbps)**。

## 🤝 贡献与许可

本项目遵循 **MIT** 协议。非常欢迎提交 Issue 或 Pull Request 以适配更多型号的舵机或优化通讯协议层。
