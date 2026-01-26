use clap::Parser;
use feetech_servo_sdk::{FeetechBus, MotorBus};
use std::time::Duration;
use tracing::{info, error, Level};
use tracing_subscriber::FmtSubscriber;

#[derive(Parser, Debug)]
#[command(author, version, about = "Scan for Feetech Servos")]
struct Args {
    /// Serial port path (e.g., /dev/ttyUSB0)
    #[arg(short, long)]
    port: String,

    /// Baud rate to scan
    #[arg(short, long, default_value_t = 1000000)]
    baud: u32,

    /// Max ID to scan (1-253)
    #[arg(long, default_value_t = 10)]
    max_id: u8,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let args = Args::parse();
    info!("Opening port {} at {} baud...", args.port, args.baud);

    let mut bus = match FeetechBus::new(&args.port, args.baud) {
        Ok(bus) => bus,
        Err(e) => {
            error!("Failed to open port: {}", e);
            return Ok(());
        }
    };

    info!("Starting scan for IDs 1 to {}...", args.max_id);
    let mut found_count = 0;

    for id in 1..=args.max_id {
        // 尝试读取位置作为 PING
        match bus.read_position(id).await {
            Ok(pos) => {
                info!("✅ Found Servo ID: {}, Position: {:.4} rad", id, pos);
                found_count += 1;
            }
            Err(feetech_servo_sdk::ServoError::Timeout { .. }) => {
                // Ignore timeouts (not found)
            }
            Err(e) => {
                error!("⚠️ Servo ID {} error: {}", id, e);
            }
        }
        // Small delay to prevent bus saturation during scan
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    info!("Scan complete. Found {} servos.", found_count);
    Ok(())
}