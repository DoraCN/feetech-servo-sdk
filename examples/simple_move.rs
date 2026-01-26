use clap::Parser;
use feetech_servo_sdk::{ControlOp, FeetechBus, MotorBus}; // 注意引入 ControlOp
use std::time::Duration;
use tracing::{Level, info};
use tracing_subscriber::FmtSubscriber;

#[derive(Parser, Debug)]
#[command(author, version, about = "Move a Feetech Servo to a specific position")]
struct Args {
    /// 串口路径
    #[arg(short, long, default_value = "/dev/ttyUSB0")]
    port: String,

    /// 波特率
    #[arg(short, long, default_value_t = 1000000)]
    baud: u32,

    /// 要控制的舵机 ID
    #[arg(short, long)]
    id: u8,

    /// 目标角度 (单位: 度, 0-360)
    #[arg(short, long)]
    degrees: f32,

    /// 动作后的等待/观察时间 (秒)
    #[arg(short, long, default_value_t = 2.0)]
    wait: f32,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let args = Args::parse();

    // 核心转换：度 -> 弧度 (SDK 使用弧度)
    let target_rad = args.degrees.to_radians();

    info!("正在连接 {}...", args.port);
    let mut bus = FeetechBus::new(&args.port, args.baud)?;

    // 1. 上电 (Enable Torque)
    // 这一步是必须的，否则电机处于“手掰模式”，即使写入位置也不会动
    info!("正在为 ID {} 上电 (Enable Torque)...", args.id);
    bus.enable_torque(&[args.id]).await?;

    // 2. 读取当前位置 (用于对比)
    let start_pos = bus.read_position(args.id).await?;
    info!("当前位置: {:.2}°", start_pos.to_degrees());

    // 3. 发送移动指令
    info!("发送目标: {:.2}° ({:.4} rad)", args.degrees, target_rad);
    // 使用 ControlOp::Position 包装目标值
    bus.write_goal(args.id, ControlOp::Position(target_rad))
        .await?;

    // 4. 等待并观察运动过程
    let check_interval = Duration::from_millis(100);
    let start_time = std::time::Instant::now();
    let wait_duration = Duration::from_secs_f32(args.wait);

    info!("正在运动...");
    while start_time.elapsed() < wait_duration {
        if let Ok(curr) = bus.read_position(args.id).await {
            // 实时打印当前位置，看是否接近目标
            info!(" >> 实时位置: {:.2}°", curr.to_degrees());
        }
        tokio::time::sleep(check_interval).await;
    }

    let final_pos = bus.read_position(args.id).await?;
    info!("最终位置: {:.2}°", final_pos.to_degrees());
    info!("完成。注意：电机仍处于上电锁力状态。");

    Ok(())
}
