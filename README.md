# Feetech Servo SDK (Rust)

[![Rust](https://img.shields.io/badge/rust-stable-brightgreen.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

这是一个高性能、基于异步 Rust (Tokio) 的飞特 (Feetech/Hiwonder) 串口总线舵机 SDK。专门设计用于控制 STS、SMS 等型号的串行总线舵机。

该项目的目标是提供一个**工业级**的控制接口，满足机器人高频控制环路（如 LeRobot、DORA 节点）对低延迟、并发安全和代码鲁棒性的核心需求。

## ✨ 核心特性

* **🚀 全异步架构**: 基于 `Tokio`，非阻塞 I/O，完美契合高性能异步控制系统。
* **🔌 I/O 解耦设计**: 核心协议引擎 `FeetechController<S>` 与具体 I/O 实现完全分离，可注入任意 `AsyncRead + AsyncWrite` 字节流（串口、USB、网络等）。
* **🛡️ 类型安全与总线互斥**: 利用 Rust 的所有权模型 (`&mut self`) 确保总线访问的互斥性，在编译期杜绝指令冲突。
* **📐 物理单位优先**: 接口层统一使用 **弧度 (Radians)**，内部自动处理 `Radians <-> Raw Ticks` 转换，简化运动学实现。
* **⚡ 批处理深度优化**: 支持硬件级同步写 (`SYNC_WRITE`)，大幅降低多关节同步控制的延迟。
* **🤖 鲁棒通讯机制**: 内置 RX 缓冲区自动排空、发送超时重试、校验和自动验证，有效抵御电气噪声干扰。
* **🧪 内置模拟后端 (Mock)**: 提供纯内存的 Mock 系统，支持物理运动模拟，方便在无硬件环境下通过特征注入进行开发。

## 📦 安装

在你的 `Cargo.toml` 中添加：

```toml
[dependencies]
feetech-servo-sdk = "0.3.0"
tokio = { version = "1", features = ["full"] }
anyhow = "1.0"
```

默认开启 `tokio-serial-impl` feature，提供开箱即用的串口支持。如需在 Android 等平台注入自定义 I/O 流，关闭默认 features：

```toml
feetech-servo-sdk = { version = "0.3.0", default-features = false }
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

### 4. 自定义 I/O 流 (Android / 跨平台)

关闭默认 features 后，可以将任意实现了 `AsyncRead + AsyncWrite + Unpin + Send` 的字节流注入到协议引擎中：

```rust
use feetech_servo_sdk::{FeetechController, MotorBus};

// your_usb_stream 可以是 Android USB 驱动、TCP socket 或任何异步字节流
let mut bus = FeetechController::from_stream(your_usb_stream);
bus.enable_torque(&[1]).await?;
```

## 🛠️ 命令行调试工具 (CLI)

SDK 内置了多个调试工具，位于 `examples/` 目录。以下是所有示例的详细说明：

### 基础工具

| 示例          | 功能                                                              | 命令                                                          |
| ------------- | ----------------------------------------------------------------- | ------------------------------------------------------------- |
| **scan**      | 扫描并发现总线上所有舵机的 ID 和位置                              | `cargo run --example scan -- -p /dev/ttyUSB0`                 |
| **set_id**    | 设置舵机 ID（使用广播 ID，适用于新舵机配置）                      | `cargo run --example set_id -- -p /dev/ttyUSB0 -n 5`          |
| **read_info** | 读取舵机详细信息（Firmware/Software版本、ID、波特率、型号、位置） | `cargo run --example read_info -- -p /dev/ttyUSB0 -i 1`       |
| **read_raw**  | 读取舵机原始位置数值（0-4095）                                    | `cargo run --example read_raw -- -p /dev/ttyUSB0 --ids 1,2,3` |
| **set_mid**   | 将当前物理位置设置为舵机中位（零点标定）                          | `cargo run --example set_mid -- -p /dev/ttyUSB0 --ids 1`      |
| **monitor**   | 实时监控舵机位置（连续读取并显示）                                | `cargo run --example monitor -- -p /dev/ttyUSB0`              |

### 控制示例

| 示例             | 功能                                       | 命令                                                                   |
| ---------------- | ------------------------------------------ | ---------------------------------------------------------------------- |
| **simple_move**  | 简单位置控制（指定角度移动）               | `cargo run --example simple_move -- -p /dev/ttyUSB0 -i 1 --degrees 90` |
| **raw_effort**   | 直接 PWM 控制（绕过位置环，原始 PWM 信号） | `cargo run --example raw_effort -- -p /dev/ttyUSB0 -i 1 --effort 2048` |
| **six_dof_move** | 6自由度机械臂平滑移动到目标位置            | `cargo run --example six_dof_move -- -p /dev/ttyUSB0`                  |

### 遥操作与高级控制

| 示例                   | 功能                                     | 命令                                                                        |
| ---------------------- | ---------------------------------------- | --------------------------------------------------------------------------- |
| **teleop**             | 主从遥操作（两个 SO-100 机械臂镜像控制） | `cargo run --example teleop -- -l /dev/ttyUSB0 -f /dev/ttyUSB1`             |
| **teleop_simple_safe** | 带安全停车功能的简单遥操作               | `cargo run --example teleop_simple_safe -- -l /dev/ttyUSB0 -f /dev/ttyUSB1` |
| **keyboard_control**   | 键盘平滑控制（方向键控制多个舵机）       | `cargo run --example keyboard_control -- -p /dev/ttyUSB0`                   |
| **omni_car**           | 全向轮小车控制（基于舵机）               | `cargo run --example omni_car -- -p /dev/ttyUSB0`                           |

### 使用示例

```bash
# 1. 扫描发现舵机
cargo run --example scan -- -p /dev/ttyUSB0

# 2. 读取舵机信息（假设发现 ID 为 6）
cargo run --example read_info -- -p /dev/ttyUSB0 -i 6

# 3. 设置新舵机 ID（假设新舵机出厂 ID 相同，需要逐个设置）
# 只连接一个舵机，断电其他舵机，然后设置
cargo run --example set_id -- -p /dev/ttyUSB0 -n 1   # 设置为 ID 1
cargo run --example set_id -- -p /dev/ttyUSB0 -n 2   # 设置为 ID 2（断电重启第一个后）
cargo run --example set_id -- -p /dev/ttyUSB0 -n 3   # 设置为 ID 3

# 4. 再次扫描确认
cargo run --example scan -- -p /dev/ttyUSB0

# 5. 简单移动测试
cargo run --example simple_move -- -p /dev/ttyUSB0 -i 1 --degrees 90

# 6. 监控位置
cargo run --example monitor -- -p /dev/ttyUSB0
```

### 模拟测试

所有示例都支持 `--features mock` 进行无硬件测试：

```bash
cargo run --example monitor --features mock -- --mock
```

## ⚙️ 硬件连接指南

* **电源**: 飞特 STS/SMS 舵机通常需要 **6V ~ 12V** 外部供电（推荐 2S 锂电 7.4V）。
* **信号**: 串行总线通常为 **单线半双工 (TTL)**。连接 Mac/PC 时，需要使用专用的信号转接板（如飞特原厂转接板或 DIY 带有三态缓冲器的电路）。
* **波特率**: 默认出厂波特率为 **1,000,000 (1Mbps)**。

## 🤝 贡献与许可

本项目遵循 **MIT** 协议。非常欢迎提交 Issue 或 Pull Request 以适配更多型号的舵机或优化通讯协议层。
