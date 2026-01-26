use clap::Parser;
use feetech_servo_sdk::{ControlOp, FeetechBus, MotorBus};
use std::time::Duration;
use tracing::{Level, error, info, warn};
use tracing_subscriber::FmtSubscriber;

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "Leader-Follower Teleoperation for two SO-100 arms"
)]
struct Args {
    /// 主手 (Leader) 的串口路径 (人操作这台)
    #[arg(long, default_value = "/dev/ttyUSB0")]
    leader_port: String,

    /// 从手 (Follower) 的串口路径 (这台跟着动)
    #[arg(long, default_value = "/dev/ttyUSB1")]
    follower_port: String,

    /// 波特率
    #[arg(long, default_value_t = 1000000)]
    baud: u32,

    /// 机械臂的关节 ID 列表
    #[arg(long, value_delimiter = ',', default_value = "1,2,3,4,5,6")]
    ids: Vec<u8>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. 初始化日志
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let args = Args::parse();

    // 2. 初始化两条总线
    // 注意：这里我们实例化了两个独立的 Bus 对象
    info!("正在连接 Leader (主手) at {}...", args.leader_port);
    let mut leader_bus = FeetechBus::new(&args.leader_port, args.baud)
        .map_err(|e| anyhow::anyhow!("无法连接 Leader: {}", e))?;

    info!("正在连接 Follower (从手) at {}...", args.follower_port);
    let mut follower_bus = FeetechBus::new(&args.follower_port, args.baud)
        .map_err(|e| anyhow::anyhow!("无法连接 Follower: {}", e))?;

    // 3. 配置机械臂状态
    // Leader 需要卸力 (Disable Torque)，以便人手操作
    // Follower 需要上电 (Enable Torque)，以便跟随动作
    info!("正在配置机械臂状态...");
    info!("Leader -> 卸力 (Disable Torque)");
    leader_bus.disable_torque(&args.ids).await?;

    info!("Follower -> 上电 (Enable Torque)");
    follower_bus.enable_torque(&args.ids).await?;

    // 4. 安全检查：同步初始位置
    // 在开始死循环之前，建议先缓慢移动 Follower 到 Leader 的当前位置，防止瞬间弹射
    info!("正在同步初始位置 (安全对齐)...");
    let initial_pos = leader_bus.sync_read_positions(&args.ids).await?;
    // 使用平滑移动或慢速移动到初始点 (这里为了简化，直接写入，但建议实操时手扶着 Follower)
    let initial_commands: Vec<(u8, ControlOp)> = args
        .ids
        .iter()
        .zip(initial_pos.iter())
        .map(|(&id, &pos)| (id, ControlOp::Position(pos)))
        .collect();
    follower_bus.sync_write_goals(&initial_commands).await?;

    // 给一点时间让 Follower 到位
    tokio::time::sleep(Duration::from_secs(1)).await;

    info!("🚀 开始遥操作 (按 Ctrl+C 停止)...");

    // 5. 控制循环 (Control Loop)
    // 目标频率：尽可能快，通常 50Hz - 100Hz 足够流畅
    let mut interval = tokio::time::interval(Duration::from_millis(10)); // 100Hz

    loop {
        interval.tick().await;

        // A. 从 Leader 读取当前角度
        // sync_read_positions 返回的是 Vec<f32> 弧度
        let positions = match leader_bus.sync_read_positions(&args.ids).await {
            Ok(pos) => pos,
            Err(e) => {
                warn!("读取 Leader 失败: {} (跳过本帧)", e);
                continue;
            }
        };

        // B. 构造写入指令
        // 直接将 Leader 的位置映射给 Follower
        // 如果两台机械臂安装方式镜像，这里可能需要对某些关节取反 (pos * -1.0)
        let commands: Vec<(u8, ControlOp)> = args
            .ids
            .iter()
            .zip(positions.iter())
            .map(|(&id, &pos)| (id, ControlOp::Position(pos)))
            .collect();

        // C. 写入 Follower
        if let Err(e) = follower_bus.sync_write_goals(&commands).await {
            error!("写入 Follower 失败: {}", e);
            // 严重错误可能需要退出或急停
        }

        // (可选) 打印调试信息
        // info!("Pos: {:.2?}", positions);
    }
}
