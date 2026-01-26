use clap::Parser;
use feetech_servo_sdk::{FeetechBus, MotorBus}; // 引入核心库
use std::time::Duration;
use tracing::{Level, error, info};
use tracing_subscriber::FmtSubscriber;

#[derive(Parser, Debug)]
#[command(author, version, about = "Monitor Feetech Servo Positions")]
struct Args {
    /// 串口路径 (例如 /dev/ttyUSB0 或 COM3)
    #[arg(short, long, default_value = "/dev/ttyUSB0")]
    port: String,

    /// 波特率
    #[arg(short, long, default_value_t = 1000000)]
    baud: u32,

    /// 要监控的舵机 ID 列表 (例如: --ids 1,2,3)
    #[arg(short, long, value_delimiter = ',')]
    ids: Vec<u8>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let args = Args::parse();

    if args.ids.is_empty() {
        error!("请提供至少一个 ID，使用 --ids 参数 (例如: --ids 1,2,3)");
        return Ok(());
    }

    info!("正在打开串口 {} (波特率: {})...", args.port, args.baud);
    // 初始化总线
    let mut bus = FeetechBus::new(&args.port, args.baud)?;

    info!("开始监控 ID: {:?} (按 Ctrl+C 停止)", args.ids);

    loop {
        // 使用 sync_read_positions 批量读取
        match bus.sync_read_positions(&args.ids).await {
            Ok(positions) => {
                // 将结果格式化为可读的字符串 (弧度 -> 度)
                let displays: Vec<String> = positions
                    .iter()
                    .zip(args.ids.iter())
                    .map(|(pos, id)| format!("ID{}: {:.1}°", id, pos.to_degrees()))
                    .collect();

                info!("Positions: {}", displays.join(" | "));
            }
            Err(e) => {
                error!("读取错误: {}", e);
                // 出错后稍微多等待一会，避免刷屏
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }

        // 50ms 刷新率 (20Hz)
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}
