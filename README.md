# Feetech Servo SDK (Rust)

[![Rust](https://img.shields.io/badge/rust-stable-brightgreen.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

这是一个专为飞特 (Feetech/Hiwonder) STS/SMS 系列串行总线舵机设计的高性能、异步 Rust 驱动库。

该项目的目标是提供一个**工业级**的控制接口，替代传统的 Python SDK，以满足高频控制环路（如 LeRobot、DORA 节点）对低延迟和并发安全的需求。

## ✨ 核心特性

* **⚡ 全异步架构**: 基于 `Tokio` 和 `tokio-serial`，非阻塞 I/O，完美契合 Rust 异步生态。
* **🛡️ 类型安全与独占所有权**: 通过 `&mut self` 强制总线独占，防止多线程竞争导致的数据包冲突。
* **📐 物理单位原生**: 摒弃 `0-100` 或 `0-4096` 的原始数值，接口层统一使用 **弧度 (Radians)**，直接对接运动学算法。
* **🚀 批处理优化**: 支持 `SYNC_WRITE` (硬件级同步写) 和 `Bulk Read` (软件级批量读)，大幅降低总线通信开销。
* **🤖 严格错误处理**: "Strict Mode" 设计，硬件报错（过压、过热、过载）会直接映射为 Rust `Err`，强制上层处理。
* **🧪 内置模拟器 (Mock)**: 提供纯内存的 Mock 后端，支持物理插值模拟，无硬件开发更轻松。
* **🛠️ 开箱即用的 CLI**: 集成扫描、监控、控制、调试工具。

## 📦 安装

在你的 `Cargo.toml` 中添加依赖：

```toml
[dependencies]
feetech-servo-sdk = { path = "." } # 如果在本地开发
# 或者使用 git 依赖
# feetech-servo-sdk = { git = "[https://github.com/your-repo/feetech-servo-sdk](https://github.com/your-repo/feetech-servo-sdk)" }

tokio = { version = "1", features = ["full"] }
anyhow = "1.0"
```

## 💻 作为库使用 (Library Usage)

### 1. 基础控制 (单个电机)

```rust
use feetech_servo_sdk::{FeetechBus, MotorBus, ControlOp};
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // 1. 初始化总线 (自动打开串口)
    let mut bus = FeetechBus::new("/dev/ttyUSB0", 1_000_000)?;

    // 2. 上电 (Enable Torque) - 必须步骤
    bus.enable_torque(&[1]).await?;

    // 3. 读取位置 (返回弧度)
    let pos = bus.read_position(1).await?;
    println!("Current Position: {:.4} rad", pos);

    // 4. 发送目标位置 (例如转到 3.14 弧度)
    bus.write_goal(1, ControlOp::Position(3.14)).await?;

    Ok(())
}

```

### 2. 高性能同步控制 (多自由度机械臂)

对于 SO100(6-DoF) 机械臂，推荐使用同步读写接口以减少延迟。

```rust
use feetech_servo_sdk::{FeetechBus, MotorBus, ControlOp};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut bus = FeetechBus::new("/dev/ttyUSB0", 1_000_000)?;
    let ids = vec![1, 2, 3, 4, 5, 6];

    // 批量上电
    bus.enable_torque(&ids).await?;

    // --- 批量读取 ---
    // 返回 Vec<f32>，顺序严格对应传入的 ids
    let positions = bus.sync_read_positions(&ids).await?;
    println!("Joints: {:?}", positions);

    // --- 批量写入 (SYNC_WRITE) ---
    // 同时发送指令，所有电机同一时刻启动
    let targets = vec![
        (1, ControlOp::Position(0.0)),
        (2, ControlOp::Position(0.5)),
        (3, ControlOp::Position(-0.5)),
        (4, ControlOp::Position(1.0)),
        (5, ControlOp::Position(0.0)),
        (6, ControlOp::Position(0.0)),
    ];
    bus.sync_write_goals(&targets).await?;

    Ok(())
}

```

### 3. 使用 Mock 模拟器

无需连接真实硬件即可测试上层逻辑。

```rust
use feetech_servo_sdk::mock::MockBus;
use feetech_servo_sdk::{MotorBus, ControlOp};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化模拟总线，包含 ID 1-6
    let mut bus = MockBus::new(&[1, 2, 3, 4, 5, 6]);

    // 调用方式与真实总线完全一致
    bus.enable_torque(&[1]).await?;
    bus.write_goal(1, ControlOp::Position(1.57)).await?;
    
    // Mock 包含简单的物理插值，立即读取可能还没到目标位
    let pos = bus.read_position(1).await?; 
    println!("Simulated Pos: {}", pos);
    
    Ok(())
}

```

## 🛠️ 命令行工具 (CLI)

本项目内置了一个强大的 CLI 工具，用于现场调试、ID 扫描和状态监控。

### 编译与运行

```bash
# 扫描总线上的舵机
cargo run --release -- scan --max-id 20

# 实时监控 ID 1, 2, 3 的位置
cargo run --release -- monitor --ids 1,2,3

# 移动 ID 1 到 90 度 (仅测试用，单位为度)
cargo run --release -- move --id 1 --degrees 90.0

# 卸力/放松电机
cargo run --release -- relax --ids 1,2,3

```

### 使用 Mock 模式运行 CLI

没有硬件也能体验：

```bash
# 启动监控模式，使用模拟后端
cargo run -- --mock monitor --ids 1,2,3

```

## ⚙️ 架构设计决策

1. **单位制**:
* **External API**: 统一使用 `f32` **弧度 (Radians)**。
* **Internal Driver**: 自动处理 `Radians <-> Raw Ticks (0-4096)` 的转换。
* *原因*: 避免上层算法（如 IK/FK）频繁进行单位转换，减少精度损失。


2. **错误处理 (Strict Mode)**:
* 当舵机返回的 Status Byte 不为 0 (例如 Overload/Overheat) 时，Rust 函数会返回 `Err(ServoError::HardwareError)`。
* *原因*: 强制开发者在代码层面处理硬件异常，防止在电机过载时继续发送指令导致烧毁。


3. **并发模型**:
* `MotorBus` trait 的方法均需要 `&mut self`。
* *原因*: 串口通信是半双工且独占的。这种设计在编译期就杜绝了“多个线程同时写串口”的 Race Condition，无需运行时加锁，性能最高。



## 🔌 硬件连接 (STS3215 / SO-100)

* **VCC**: 6V - 8.4V (推荐 7.4V 2S 锂电)
* **GND**: 共地
* **DATA**: 连接到 USB 转 TTL 模块的 TX/RX (STS 系列通常只需要单线半双工，需配合信号转接板)
* **Baud Rate**: 默认 1,000,000 (1Mbps)

## 🤝 贡献与协议

本项目遵循 MIT 协议。欢迎提交 PR 适配更多型号的飞特舵机或增加 Protocol 1.0 支持。
