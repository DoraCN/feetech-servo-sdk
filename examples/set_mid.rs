use clap::Parser;
use feetech_servo_sdk::{FeetechBus, MotorBus};
use std::time::Duration;
use tracing::{Level, error, info};
use tracing_subscriber::FmtSubscriber;

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "Set current position of Feetech Servos as middle position"
)]
struct Args {
    /// Serial port path
    #[arg(short, long, default_value = "/dev/ttyUSB0")]
    port: String,

    /// Baud rate
    #[arg(short, long, default_value_t = 1000000)]
    baud: u32,

    /// Servo IDs to calibrate (comma separated, e.g., "1,2,3")
    #[arg(long, value_delimiter = ',', default_value = "1,2,3,4,5,6")]
    ids: Vec<u8>,

    /// Use Mock backend instead of real hardware
    #[arg(long)]
    mock: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let args = Args::parse();

    let mut bus: Box<dyn MotorBus> = if args.mock {
        info!("Running in MOCK mode for IDs: {:?}", args.ids);
        #[cfg(feature = "mock")]
        {
            use feetech_servo_sdk::mock::MockBus;
            Box::new(MockBus::new(&args.ids))
        }
        #[cfg(not(feature = "mock"))]
        {
            anyhow::bail!("Mock feature not enabled");
        }
    } else {
        info!("Connecting to {}...", args.port);
        Box::new(FeetechBus::new(&args.port, args.baud)?)
    };

    info!(
        "Setting current positions as middle for IDs: {:?}",
        args.ids
    );

    for &id in &args.ids {
        match bus.set_middle_position(id).await {
            Ok(_) => info!("✅ ID {} set to middle position successfully.", id),
            Err(e) => error!("❌ Failed to set ID {} to middle: {}", id, e),
        }
        // [修复] 增加延时，校准指令涉及 NVM 写入或内部重置，需要时间处理
        // if !args.mock {
        //     tokio::time::sleep(Duration::from_millis(200)).await;
        // }
    }

    info!("Calibration complete.");

    Ok(())
}
