use clap::Parser;
use feetech_servo_sdk::{ControlOp, FeetechBus, MotorBus};
use std::time::Duration;
use tracing::{Level, info};
use tracing_subscriber::FmtSubscriber;

const SAFE_PARK_POSE: [f32; 6] = [90.0, 45.0, -45.0, 0.0, 0.0, 0.0];

#[derive(Parser, Debug)]
#[command(author, version, about = "Move Arm to Zero and Return to Park Position")]
struct Args {
    /// 串口路径
    #[arg(short, long, default_value = "/dev/ttyUSB0")]
    port: String,

    /// 波特率
    #[arg(short, long, default_value_t = 1000000)]
    baud: u32,

    /// 在零位等待时间 (秒)
    #[arg(short, long, default_value_t = 5.0)]
    wait: f32,

    /// 运动耗时 (秒)，时间越长速度越慢
    #[arg(short, long, default_value_t = 3.0)]
    duration: f32,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. 初始化日志
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let args = Args::parse();

    // 假设机械臂的 ID 是 1 到 6
    let arm_ids = vec![1, 2, 3, 4, 5, 6];

    info!(
        "正在连接机械臂 (Port: {}, Baud: {})...",
        args.port, args.baud
    );
    let mut bus = FeetechBus::new(&args.port, args.baud)?;

    // 2. 全体上电 (Enable Torque)
    // 这一步是必须的，保证到达位置后能“锁定”
    info!("正在上电并锁定关节...");
    bus.enable_torque(&arm_ids).await?;

    // 3. 定义目标姿态 (单位：度)
    // 零位姿态
    let zero_pose_deg = vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0];

    info!("准备移动到零位: {:?}", zero_pose_deg);
    info!("计划耗时: {:.1} 秒", args.duration);

    // 4. 执行平滑移动到零位
    move_smoothly(&mut bus, &arm_ids, &zero_pose_deg, args.duration).await?;

    info!("✅ 已到达零位，等待 {:.1} 秒...", args.wait);
    tokio::time::sleep(Duration::from_secs_f32(args.wait)).await;

    // 5. 回到安全停机位置
    info!("正在回到安全停机位置: {:?}", SAFE_PARK_POSE);
    move_smoothly(&mut bus, &arm_ids, &SAFE_PARK_POSE.to_vec(), args.duration).await?;

    // 6. 等待稳定后卸力并退出
    info!("🔓 正在卸力并退出...");
    bus.disable_torque(&arm_ids).await?;

    info!("程序结束。");

    Ok(())
}

/// 平滑移动函数
/// 通过软件插值，将动作分解为多个小步骤，从而控制速度
async fn move_smoothly(
    bus: &mut FeetechBus,
    ids: &[u8],
    target_deg: &[f32],
    duration_sec: f32,
) -> anyhow::Result<()> {
    if ids.len() != target_deg.len() {
        return Err(anyhow::anyhow!("ID 数量与角度数量不匹配"));
    }

    // 1. 获取起始位置 (同步读取)
    let start_rads = bus.sync_read_positions(ids).await?;
    let target_rads: Vec<f32> = target_deg.iter().map(|d| d.to_radians()).collect();

    // 2. 计算插值步数
    // 假设控制频率为 50Hz (每 20ms 发送一次指令)
    let frequency = 50.0;
    let steps = (duration_sec * frequency) as usize;
    let dt = Duration::from_secs_f32(1.0 / frequency);

    info!("开始插值运动: 总步数 {}", steps);

    for step in 1..=steps {
        // 计算当前进度的百分比 (0.0 ~ 1.0)
        let t = step as f32 / steps as f32;

        // 使用简单的线性插值 (Lerp)
        // 如果想要更平滑，可以使用 Ease-In-Out 曲线
        let current_targets: Vec<(u8, ControlOp)> = ids
            .iter()
            .zip(start_rads.iter())
            .zip(target_rads.iter())
            .map(|((&id, &start), &end)| {
                let interpolated_rad = start + (end - start) * t;
                (id, ControlOp::Position(interpolated_rad))
            })
            .collect();

        // 发送同步写指令
        bus.sync_write_goals(&current_targets).await?;

        // 等待下一个周期
        tokio::time::sleep(dt).await;
    }

    Ok(())
}
