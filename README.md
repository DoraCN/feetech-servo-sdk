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

SDK 内置了多个调试工具，位于 `examples/` 目录。以下是所有示例的详细参数说明：

---

### scan - 扫描舵机

扫描并发现总线上所有舵机的 ID 和位置。

**命令：**
```bash
cargo run --example scan -- [参数]
```

**参数：**

| 参数       | 短参数 | 参数类型 | 默认值         | 必填 | 说明                                                       |
| ---------- | ------ | -------- | -------------- | ---- | ---------------------------------------------------------- |
| `--port`   | `-p`   | String   | `/dev/ttyUSB0` | 否   | 串口路径，Linux/macOS 为 `/dev/ttyUSB0`，Windows 为 `COM3` |
| `--baud`   | `-b`   | u32      | `1000000`      | 否   | 波特率，默认 1Mbps                                         |
| `--max-id` | -      | u8       | `10`           | 否   | 最大扫描 ID 范围 (1-253)                                   |

**示例：**
```bash
cargo run --example scan -- -p /dev/ttyUSB0                    # 默认扫描 ID 1-10
cargo run --example scan -- -p /dev/ttyUSB0 --max-id 20         # 扫描 ID 1-20
cargo run --example scan -- -p /dev/ttyUSB0 -b 115200           # 使用 115200 波特率
```

---

### set_id - 设置舵机 ID

设置舵机 ID（使用广播 ID 0xFE，适用于新舵机配置）。**注意：设置后需要断电重启舵机才能生效。**

**命令：**
```bash
cargo run --example set_id -- [参数]
```

**参数：**

| 参数              | 短参数 | 参数类型     | 默认值         | 必填   | 说明                                                                              |
| ----------------- | ------ | ------------ | -------------- | ------ | --------------------------------------------------------------------------------- |
| `--port`          | `-p`   | String       | `/dev/ttyUSB0` | 否     | 串口路径                                                                          |
| `--baud`          | `-b`   | u32          | `1000000`      | 否     | 波特率                                                                            |
| `--new-id`        | `-n`   | u8           | -              | **是** | 要设置的新 ID (1-253)，**0xFE (254) 为广播 ID 不可用**                            |
| `--baudrate-code` | -      | Option\<u8\> | -              | 否     | 同时设置波特率：0=1M, 1=500K, 2=250K, 3=128K, 4=115200, 5=76800, 6=57600, 7=38400 |

**示例：**
```bash
cargo run --example set_id -- -p /dev/ttyUSB0 -n 5              # 设置为 ID 5
cargo run --example set_id -- -p /dev/ttyUSB0 -n 1 --baudrate-code 4  # 设置 ID 1 并改为 115200 波特率
```

**使用流程：**
1. 只连接一个舵机（其他断电）
2. 运行命令设置新 ID
3. **断电重启舵机**（必须！）
4. 连接下一个舵机，重复步骤 2-3
5. 全部设置完成后用 `scan` 验证

---

### read_info - 读取舵机信息

读取舵机详细信息，包括 Firmware/Software 版本、ID、波特率、型号、当前位置等。

**命令：**
```bash
cargo run --example read_info -- [参数]
```

**参数：**

| 参数     | 短参数 | 参数类型 | 默认值         | 必填 | 说明            |
| -------- | ------ | -------- | -------------- | ---- | --------------- |
| `--port` | `-p`   | String   | `/dev/ttyUSB0` | 否   | 串口路径        |
| `--baud` | `-b`   | u32      | `1000000`      | 否   | 波特率          |
| `--id`   | `-i`   | u8       | `1`            | 否   | 要读取的舵机 ID |

**示例：**
```bash
cargo run --example read_info -- -p /dev/ttyUSB0 -i 6          # 读取 ID 6 的信息
cargo run --example read_info -- -p /dev/ttyUSB0                # 默认读取 ID 1
```

**输出信息：**
- Firmware Version（固件版本）
- Software Version（软件版本）
- ID（当前 ID）
- Baud Rate（波特率）
- Model（型号代码）
- Current Position（当前角度，弧度和度）
- Raw Position（原始 tick 值，0-4095）

---

### read_raw - 读取原始位置

读取舵机原始位置数值（0-4095 tick）。

**命令：**
```bash
cargo run --example read_raw -- [参数]
```

**参数：**

| 参数     | 短参数 | 参数类型  | 默认值         | 必填 | 说明                           |
| -------- | ------ | --------- | -------------- | ---- | ------------------------------ |
| `--port` | `-p`   | String    | `/dev/ttyUSB0` | 否   | 串口路径                       |
| `--baud` | `-b`   | u32       | `1000000`      | 否   | 波特率                         |
| `--ids`  | -      | Vec\<u8\> | `1,2,3,4,5,6`  | 否   | 要读取的舵机 ID 列表，逗号分隔 |

**示例：**
```bash
cargo run --example read_raw -- -p /dev/ttyUSB0 --ids 1,2,3    # 读取 ID 1,2,3
cargo run --example read_raw -- -p /dev/ttyUSB0                 # 默认读取 1-6
```

---

### set_mid - 设置中位（零点标定）

将当前物理位置设置为舵机中位（零点标定），用于校准舵机的机械零位。

**命令：**
```bash
cargo run --example set_mid -- [参数]
```

**参数：**

| 参数     | 短参数 | 参数类型  | 默认值         | 必填 | 说明                           |
| -------- | ------ | --------- | -------------- | ---- | ------------------------------ |
| `--port` | `-p`   | String    | `/dev/ttyUSB0` | 否   | 串口路径                       |
| `--baud` | `-b`   | u32       | `1000000`      | 否   | 波特率                         |
| `--ids`  | -      | Vec\<u8\> | `1,2,3,4,5,6`  | 否   | 要校准的舵机 ID 列表，逗号分隔 |
| `--mock` | -      | bool      | -              | 否   | 使用 Mock 后端进行测试         |

**示例：**
```bash
cargo run --example set_mid -- -p /dev/ttyUSB0 --ids 1,2       # 校准 ID 1 和 2
cargo run --example set_mid -- -p /dev/ttyUSB0                   # 默认校准 1-6
cargo run --example set_mid --features mock -- --mock            # 使用模拟测试
```

---

### monitor - 监控位置

实时监控舵机位置，连续读取并显示。

**命令：**
```bash
cargo run --example monitor -- [参数]
```

**参数：**

| 参数     | 短参数 | 参数类型  | 默认值         | 必填   | 说明                           |
| -------- | ------ | --------- | -------------- | ------ | ------------------------------ |
| `--port` | `-p`   | String    | `/dev/ttyUSB0` | 否     | 串口路径                       |
| `--baud` | `-b`   | u32       | `1000000`      | 否     | 波特率                         |
| `--ids`  | -      | Vec\<u8\> | -              | **是** | 要监控的舵机 ID 列表，逗号分隔 |

**示例：**
```bash
cargo run --example monitor -- -p /dev/ttyUSB0 --ids 1,2,3     # 监控 ID 1,2,3
cargo run --example monitor -- -p /dev/ttyUSB0 -i 1             # 监控单个 ID 1
```

---

### simple_move - 简单移动

简单位置控制，将舵机移动到指定角度。

**命令：**
```bash
cargo run --example simple_move -- [参数]
```

**参数：**

| 参数        | 短参数 | 参数类型 | 默认值         | 必填   | 说明                       |
| ----------- | ------ | -------- | -------------- | ------ | -------------------------- |
| `--port`    | `-p`   | String   | `/dev/ttyUSB0` | 否     | 串口路径                   |
| `--baud`    | `-b`   | u32      | `1000000`      | 否     | 波特率                     |
| `--id`      | `-i`   | u8       | -              | **是** | 要控制的舵机 ID            |
| `--degrees` | `-d`   | f32      | -              | **是** | 目标角度（度），范围 0-360 |
| `--wait`    | `-w`   | f32      | `2.0`          | 否     | 移动后等待观察时间（秒）   |

**示例：**
```bash
cargo run --example simple_move -- -p /dev/ttyUSB0 -i 1 -d 90      # 移动到 90 度
cargo run --example simple_move -- -p /dev/ttyUSB0 -i 2 -d 180 -w 5 # 移动到 180 度，等待 5 秒
```

---

### raw_effort - PWM 控制

直接 PWM 控制，绕过位置环，使用原始 PWM 信号驱动舵机。

**命令：**
```bash
cargo run --example raw_effort -- [参数]
```

**参数：**

| 参数         | 短参数 | 参数类型  | 默认值         | 必填 | 说明                           |
| ------------ | ------ | --------- | -------------- | ---- | ------------------------------ |
| `--port`     | `-p`   | String    | `/dev/ttyUSB0` | 否   | 串口路径                       |
| `--baud`     | `-b`   | u32       | `1000000`      | 否   | 波特率                         |
| `--ids`      | -      | Vec\<u8\> | `1,2,3,4,5,6`  | 否   | 要控制的舵机 ID 列表，逗号分隔 |
| `--effort`   | `-e`   | i16       | `200`          | 否   | PWM 值，范围约 -4096 到 4096   |
| `--duration` | `-d`   | f32       | `2.0`          | 否   | 运行时长（秒）                 |
| `--mock`     | -      | bool      | -              | 否   | 使用 Mock 后端进行测试         |

**示例：**
```bash
cargo run --example raw_effort -- -p /dev/ttyUSB0 --ids 1 -e 500 -d 3
cargo run --example raw_effort -- -p /dev/ttyUSB0 -e 200                   # 默认参数
```

---

### to_zero - 走零位并自动复位

将机械臂移动到零位，等待后自动回到安全停机位置，然后卸力退出。安全停机位置为 `[0.0, -107.7, 91.6, 64.0, -0.3, 0.0]`。

**命令：**
```bash
cargo run --example to_zero -- [参数]
```

**参数：**

| 参数         | 短参数 | 参数类型 | 默认值         | 必填 | 说明                             |
| ------------ | ------ | -------- | -------------- | ---- | -------------------------------- |
| `--port`     | `-p`   | String   | `/dev/ttyUSB0` | 否   | 串口路径                         |
| `--baud`     | `-b`   | u32      | `1000000`      | 否   | 波特率                           |
| `--wait`     | `-w`   | f32      | `5.0`          | 否   | 在零位等待时间（秒）             |
| `--duration` | `-d`   | f32      | `3.0`          | 否   | 运动耗时（秒），时间越长速度越慢 |

**示例：**
```bash
cargo run --example to_zero -- -p /dev/ttyUSB0                    # 默认：零位等待 5 秒
cargo run --example to_zero -- -p /dev/ttyUSB0 -w 10                # 零位等待 10 秒
cargo run --example to_zero -- -p /dev/ttyUSB0 -d 5                  # 运动耗时 5 秒，更慢
```

**流程：**
1. 移动到零位 `[0°, 0°, 0°, 0°, 0°, 0°]`
2. 等待指定时间
3. 移动到安全停机位置 `[0.0, -107.7, 91.6, 64.0, -0.3, 0.0]`
4. 卸力并退出程序

---

### teleop - 主从遥操作

主从遥操作，控制两个 SO-100 机械臂镜像运动。

**命令：**
```bash
cargo run --example teleop -- [参数]
```

**参数：**

| 参数              | 短参数 | 参数类型  | 默认值         | 必填 | 说明                                     |
| ----------------- | ------ | --------- | -------------- | ---- | ---------------------------------------- |
| `--leader-port`   | -      | String    | `/dev/ttyUSB0` | 否   | 主手（Leader）串口路径，人操作的舵机     |
| `--follower-port` | -      | String    | `/dev/ttyUSB1` | 否   | 从手（Follower）串口路径，跟随运动的舵机 |
| `--baud`          | -      | u32       | `1000000`      | 否   | 波特率                                   |
| `--ids`           | -      | Vec\<u8\> | `1,2,3,4,5,6`  | 否   | 机械臂关节 ID 列表，逗号分隔             |

**示例：**
```bash
cargo run --example teleop -- --leader-port /dev/ttyUSB0 --follower-port /dev/ttyUSB1
cargo run --example teleop -- -l /dev/ttyUSB0 -f /dev/ttyUSB1 --ids 1,2,3,4
```

---

### teleop_simple_safe - 安全遥操作

带安全停车功能的简单遥操作，当停止操作时舵机会自动归位。

**命令：**
```bash
cargo run --example teleop_simple_safe -- [参数]
```

**参数：**

| 参数              | 短参数 | 参数类型  | 默认值         | 必填 | 说明                                 |
| ----------------- | ------ | --------- | -------------- | ---- | ------------------------------------ |
| `--leader-port`   | -      | String    | `/dev/ttyUSB0` | 否   | 主手（Leader）串口路径               |
| `--follower-port` | -      | String    | `/dev/ttyUSB1` | 否   | 从手（Follower）串口路径             |
| `--ids`           | -      | Vec\<u8\> | `1,2,3,4,5,6`  | 否   | 关节 ID 列表，逗号分隔               |
| `--park-duration` | -      | f32       | `5.0`          | 否   | 归位运动总耗时（秒），时间越长越安全 |

**示例：**
```bash
cargo run --example teleop_simple_safe -- -l /dev/ttyUSB0 -f /dev/ttyUSB1
cargo run --example teleop_simple_safe -- --park-duration 10   # 10 秒归位，更慢更安全
```

---

### keyboard_control - 键盘控制

使用键盘方向键平滑控制多个舵机。

**命令：**
```bash
cargo run --example keyboard_control -- [参数]
```

**参数：**

| 参数           | 短参数 | 参数类型 | 默认值         | 必填 | 说明              |
| -------------- | ------ | -------- | -------------- | ---- | ----------------- |
| `--port`       | `-p`   | String   | `/dev/ttyUSB0` | 否   | 串口路径          |
| `--speed`      | -      | f32      | `800.0`        | 否   | 移动速度 (0-1500) |
| `--turn-speed` | -      | f32      | `500.0`        | 否   | 旋转速度 (0-1500) |

**示例：**
```bash
cargo run --example keyboard_control -- -p /dev/ttyUSB0
cargo run --example keyboard_control -- -p /dev/ttyUSB0 --speed 1000
```

---

### omni_car - 全向轮小车

控制基于舵机的全向轮小车。

**命令：**
```bash
cargo run --example omni_car -- [参数]
```

**参数：**

| 参数     | 短参数 | 参数类型 | 默认值         | 必填 | 说明     |
| -------- | ------ | -------- | -------------- | ---- | -------- |
| `--port` | `-p`   | String   | `/dev/ttyUSB0` | 否   | 串口路径 |

**示例：**
```bash
cargo run --example omni_car -- -p /dev/ttyUSB0
```

---

### 使用示例流程

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
cargo run --example simple_move -- -p /dev/ttyUSB0 -i 1 -d 90

# 6. 监控位置
cargo run --example monitor -- -p /dev/ttyUSB0 --ids 1,2,3
```

---

### 模拟测试

所有示例都支持 `--features mock` 进行无硬件测试（需要硬件的示例可加 `--mock` 参数）：

```bash
cargo run --example monitor --features mock -- --mock
cargo run --example set_mid --features mock -- --mock
```

## ⚙️ 硬件连接指南

* **电源**: 飞特 STS/SMS 舵机通常需要 **6V ~ 12V** 外部供电（推荐 2S 锂电 7.4V）。
* **信号**: 串行总线通常为 **单线半双工 (TTL)**。连接 Mac/PC 时，需要使用专用的信号转接板（如飞特原厂转接板或 DIY 带有三态缓冲器的电路）。
* **波特率**: 默认出厂波特率为 **1,000,000 (1Mbps)**。

## 🤝 贡献与许可

本项目遵循 **MIT** 协议。非常欢迎提交 Issue 或 Pull Request 以适配更多型号的舵机或优化通讯协议层。
