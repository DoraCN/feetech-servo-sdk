use anyhow::Result;
use clap::{Parser, Subcommand};
use feetech_servo_sdk::{ControlOp, FeetechBus, MotorBus};
// 如果你想把 mock 暴露给 bin，需要在 lib.rs 设为 pub mod mock，或者在这里把 mock 代码作为模块引入
// 假设在 lib.rs 中已经 pub mod mock;
// use feetech_servo_sdk::mock::MockBus;
// *注意*: 为了让这段代码现在就能跑，我会在 main.rs 里根据 feature flag 动态处理，
// 或者在真实项目中，建议把 MockBus 放在 lib.rs 导出。

use std::time::Duration;
use tracing::{Level, error, info};
use tracing_subscriber::FmtSubscriber;

#[derive(Parser)]
#[command(author, version, about = "Feetech Servo SDK CLI Tool")]
struct Cli {
    /// Serial port (e.g., /dev/ttyUSB0 or COM3). Ignored if --mock is set.
    #[arg(short, long, default_value = "/dev/ttyUSB0")]
    port: String,

    /// Baud rate
    #[arg(short, long, default_value_t = 1000000)]
    baud: u32,

    /// Use Mock backend instead of real hardware
    #[arg(long)]
    mock: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Scan for connected servos (Ping ID 1-253)
    Scan {
        #[arg(long, default_value_t = 20)]
        max_id: u8,
    },
    /// Continuously monitor positions of specific IDs
    Monitor {
        /// Servo IDs to monitor (comma separated, e.g., "1,2,3")
        // 👇 修改这里：添加 short, long
        #[arg(short, long, value_delimiter = ',')]
        ids: Vec<u8>,
    },
    /// Move a servo to a specific position
    Move {
        /// Servo ID
        #[arg(short, long)] // 建议这里也加上
        id: u8,
        /// Target position in DEGREES (0-360)
        #[arg(short, long)] // 建议这里也加上
        degrees: f32,
        /// Execution time in seconds (optional wait)
        #[arg(short, long, default_value_t = 0.0)]
        time: f32,
    },
    /// Reset/Relax (Disable Torque)
    Relax {
        // 👇 修改这里：添加 short, long
        #[arg(short, long, value_delimiter = ',')]
        ids: Vec<u8>,
    },
}

// 辅助函数：将 Box<dyn MotorBus> 用于多态
// 这样我们可以透明地切换 Real 和 Mock
async fn get_bus(args: &Cli) -> Result<Box<dyn MotorBus>> {
    if args.mock {
        info!("🤖 Initializing MOCK bus...");
        // 假设 MockBus 在 lib 中可用，或者我们可以为了演示创建一个临时的
        // 这里假设你在 lib.rs 导出了 mock::MockBus
        // let bus = feetech_servo_sdk::mock::MockBus::new(&[1, 2, 3, 4, 5, 6]);
        // 为了演示编译通过，这里我们暂时无法直接调用 lib 内部的 mock，
        // 实际开发时请在 lib.rs 添加 `pub mod mock;`
        panic!("Mock bus requires `pub mod mock` in lib.rs. Please enable it.");
    } else {
        info!("🔌 Opening serial port {} at {}...", args.port, args.baud);
        let bus = FeetechBus::new(&args.port, args.baud)?;
        Ok(Box::new(bus))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // 设置日志
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let cli = Cli::parse();

    // 在这里初始化总线
    // 注意：Rust 的 trait object 需要处理 Send/Sync，我们的 Trait 已经继承了
    // 为了简化 main.rs 的依赖，这里我们直接根据 mock flag 分支
    // 如果是 Mock 模式，我们手动构造 mock；如果是实物，构造 FeetechBus

    // 由于 Rust 的类型系统，我们需要把它们统一包装成 Box<dyn MotorBus>
    // 但为了 Antigravity 的直接运行，我们先只处理 FeetechBus 的路径，
    // 如果你需要测试 Mock，请确保在 lib.rs 开放了接口。

    let mut bus: Box<dyn MotorBus> = if cli.mock {
        // 这里我们做个小 Hack，如果是在 Antigravity 单文件里跑不通，
        // 实际项目中请使用 use feetech_servo_sdk::mock::MockBus;
        info!("Running in MOCK mode (Simulating IDs 1-6)");
        // 这里的 ids 只是为了 mock 初始化预设
        // 实际代码需解开下行注释
        // Box::new(feetech_servo_sdk::mock::MockBus::new(&[1, 2, 3, 4, 5, 6]))
        panic!("To run mock from CLI, export MockBus in lib.rs");
    } else {
        Box::new(FeetechBus::new(&cli.port, cli.baud)?)
    };

    match &cli.command {
        Commands::Scan { max_id } => {
            info!("Starting scan up to ID {}...", max_id);
            let mut found = Vec::new();
            for id in 1..=*max_id {
                // 尝试读位置，如果成功就认为存在
                match bus.read_position(id).await {
                    Ok(pos) => {
                        let deg = pos.to_degrees();
                        info!("✅ Found ID {}: {:.2}° ({:.4} rad)", id, deg, pos);
                        found.push(id);
                    }
                    Err(_) => {
                        // 忽略超时
                    }
                }
                // 扫描时稍微 sleep 一下防止总线拥塞
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            info!("Scan complete. Found {} devices: {:?}", found.len(), found);
        }

        Commands::Monitor { ids } => {
            if ids.is_empty() {
                error!("Please provide at least one ID to monitor");
                return Ok(());
            }
            info!("Monitoring IDs: {:?}. Press Ctrl+C to stop.", ids);
            loop {
                // 使用 sync_read 批量读取
                match bus.sync_read_positions(ids).await {
                    Ok(positions) => {
                        let displays: Vec<String> = positions
                            .iter()
                            .zip(ids.iter())
                            .map(|(pos, id)| format!("ID{}: {:.1}°", id, pos.to_degrees()))
                            .collect();
                        info!("Positions: {}", displays.join(" | "));
                    }
                    Err(e) => error!("Read error: {}", e),
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }

        Commands::Move { id, degrees, time } => {
            let rad = degrees.to_radians();
            info!("Moving ID {} to {:.2}° ({:.4} rad)...", id, degrees, rad);

            // 1. 上电
            bus.enable_torque(&[*id]).await?;

            // 2. 发送目标
            bus.write_goal(*id, ControlOp::Position(rad)).await?;

            info!("Command sent.");
            if *time > 0.0 {
                tokio::time::sleep(Duration::from_secs_f32(*time)).await;
            }
        }

        Commands::Relax { ids } => {
            info!("Disabling torque for IDs: {:?}", ids);
            bus.disable_torque(ids).await?;
            info!("Done.");
        }
    }

    Ok(())
}
