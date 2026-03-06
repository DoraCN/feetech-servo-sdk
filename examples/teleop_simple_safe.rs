use clap::Parser;
use feetech_servo_sdk::{ControlOp, FeetechBus, MotorBus};
use std::time::Duration;
use tracing::{Level, error, info, warn};
use tracing_subscriber::FmtSubscriber;

// ==========================================
// 🔧 定义安全归位位姿 (弧度)
// ==========================================
// 0.0 通常表示居中或伸直。请根据实际机械臂构型修改。
const SAFE_PARK_POSE: [f32; 6] = [
    0.0,    // ID 1: 底座
    -105.0, // ID 2: 肩部 (稍微抬起)
    90.0,   // ID 3: 肘部 (稍微弯曲)
    74.0,   // ID 4: 腕旋
    0.0,    // ID 5: 腕弯
    0.0,    // ID 6: 夹爪
];

#[derive(Parser, Debug)]
#[command(author, version, about = "Simple Teleop with Safe Parking")]
struct Args {
    #[arg(long, default_value = "/dev/ttyUSB0")]
    leader_port: String,

    #[arg(long, default_value = "/dev/ttyUSB1")]
    follower_port: String,

    #[arg(long, value_delimiter = ',', default_value = "1,2,3,4,5,6")]
    ids: Vec<u8>,

    /// 归位时的运动总耗时 (秒)
    /// 时间越长，速度越慢，越安全
    #[arg(long, default_value_t = 5.0)]
    park_duration: f32,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. 初始化日志
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;
    let args = Args::parse();

    // 2. 连接设备
    info!("🔗 连接主臂 (Leader)...");
    let mut leader = FeetechBus::new(&args.leader_port, 1_000_000)?;

    info!("🔗 连接从臂 (Follower)...");
    let mut follower = FeetechBus::new(&args.follower_port, 1_000_000)?;

    // 3. 配置状态 (最基础的遥操作逻辑)
    info!("⚙️ 配置机械臂状态...");

    // 主臂 -> 卸力 (Disable Torque)
    // 这样你就可以用手随意掰动它，没有任何阻力
    leader.disable_torque(&args.ids).await?;

    // 从臂 -> 上电 (Enable Torque)
    // 这样它就会有力气去模仿主臂
    follower.enable_torque(&args.ids).await?;

    // 4. 安全对齐
    // 在开始之前，先把从臂移动到主臂当前的位置，防止瞬间弹射
    info!("🔄 同步初始位置...");
    let start_pos = leader.sync_read_positions(&args.ids).await?;
    let start_cmds: Vec<(u8, ControlOp)> = args
        .ids
        .iter()
        .zip(start_pos.iter())
        .map(|(&id, &pos)| (id, ControlOp::Position(pos)))
        .collect();
    follower.sync_write_goals(&start_cmds).await?;

    info!("🚀 开始遥操作 (按 Ctrl+C 停止并归位)...");

    let mut interval = tokio::time::interval(Duration::from_millis(10)); // 100Hz
    let mut ctrl_c = std::pin::pin!(tokio::signal::ctrl_c());

    // --- 主循环 ---
    loop {
        tokio::select! {
            // 任务 A: 遥操作循环
            _ = interval.tick() => {
                // 1. 读取主臂位置
                // 主臂是卸力的，位置由你的手决定
                let positions = match leader.sync_read_positions(&args.ids).await {
                    Ok(p) => p,
                    Err(e) => {
                        warn!("主臂读取失败: {}", e);
                        continue;
                    }
                };

                // 2. 写入从臂
                // 将读取到的位置直接发给从臂
                let commands: Vec<(u8, ControlOp)> = args.ids.iter()
                    .zip(positions.iter())
                    .map(|(&id, &pos)| (id, ControlOp::Position(pos)))
                    .collect();

                if let Err(e) = follower.sync_write_goals(&commands).await {
                    error!("从臂写入失败: {}", e);
                }
            }

            // 任务 B: 监听退出信号
            res = &mut ctrl_c => {
                if res.is_ok() {
                    info!("🛑 收到退出信号，接管控制权...");
                    break; // 跳出循环，进入下方的归位逻辑
                }
            }
        }
    }

    // ==========================================
    //          安全归位逻辑 (Safe Parking)
    // ==========================================
    info!(
        "🏠 正在自动归位到: {:?} (耗时 {}s)...",
        SAFE_PARK_POSE, args.park_duration
    );

    // 1. 关键步骤：主臂上电！
    // 刚才主臂是卸力的(软的)，现在我们要让它自动动起来，必须先上电
    info!("⚡ 主臂上电 (准备自动移动)...");
    leader.enable_torque(&args.ids).await?;

    // 从臂本来就是上电的，保持即可

    let _ = leader.disable_torque(&args.ids).await;

    // 4. 执行平滑移动
    move_smoothly(
        &mut follower,
        &args.ids,
        &SAFE_PARK_POSE,
        args.park_duration,
    )
    .await?;

    // 2. 获取起点
    // let start_leader = leader
    //     .sync_read_positions(&args.ids)
    //     .await
    //     .unwrap_or(vec![0.0; 6]);
    // let start_follower = follower
    //     .sync_read_positions(&args.ids)
    //     .await
    //     .unwrap_or(vec![0.0; 6]);
    // let target_pose = SAFE_PARK_POSE;

    // // 3. 执行插值运动
    // // 这是一个纯软件的轨迹规划，让电机慢慢动过去
    // let hz = 50.0;
    // let total_steps = (args.park_duration * hz) as usize;
    // let dt = Duration::from_secs_f32(1.0 / hz);

    // for step in 1..=total_steps {
    //     let t = step as f32 / total_steps as f32; // 0.0 -> 1.0 (进度)

    //     // 简单的线性插值 (Lerp)
    //     // Current = Start + (Target - Start) * t

    //     // 计算主臂
    //     let l_cmds: Vec<(u8, ControlOp)> = args
    //         .ids
    //         .iter()
    //         .zip(start_leader.iter())
    //         .zip(target_pose.iter())
    //         .map(|((&id, &start), &target)| {
    //             let curr = start + (target - start) * t;
    //             (id, ControlOp::Position(curr))
    //         })
    //         .collect();

    //     // 计算从臂
    //     let f_cmds: Vec<(u8, ControlOp)> = args
    //         .ids
    //         .iter()
    //         .zip(start_follower.iter())
    //         .zip(target_pose.iter())
    //         .map(|((&id, &start), &target)| {
    //             let curr = start + (target - start) * t;
    //             (id, ControlOp::Position(curr))
    //         })
    //         .collect();

    //     // 发送指令
    //     let _ = leader.sync_write_goals(&l_cmds).await;
    //     let _ = follower.sync_write_goals(&f_cmds).await;

    //     tokio::time::sleep(dt).await;
    // }

    info!("✅ 已到达安全位姿。");

    // ==========================================
    //             最终关机 (Shutdown)
    // ==========================================
    info!("💤 安全卸力 (Disable Torque)...");

    // 此时两个手臂都已经到了 0 位，我们可以放心地断电了
    // let _ = leader.disable_torque(&args.ids).await;
    let _ = follower.disable_torque(&args.ids).await;

    info!("👋 程序退出。");
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
