use clap::Parser;
use feetech_servo_sdk::{FeetechBus, MotorBus};
use tracing::{Level, info};
use tracing_subscriber::FmtSubscriber;

#[derive(Parser, Debug)]
#[command(author, version, about = "Read raw position from a Feetech Servo")]
struct Args {
    #[arg(short, long, default_value = "/dev/ttyUSB0")]
    port: String,

    #[arg(short, long, default_value_t = 1000000)]
    baud: u32,

    #[arg(short, long)]
    id: u8,

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
        info!("Running in MOCK mode (ID {})", args.id);
        #[cfg(feature = "mock")]
        {
            use feetech_servo_sdk::mock::MockBus;
            Box::new(MockBus::new(&[args.id]))
        }
        #[cfg(not(feature = "mock"))]
        {
            anyhow::bail!("Mock feature not enabled");
        }
    } else {
        info!("Connecting to {}...", args.port);
        Box::new(FeetechBus::new(&args.port, args.baud)?)
    };

    info!("Reading ID {}...", args.id);

    match bus.read_position(args.id).await {
        Ok(pos) => info!(
            "Current Position (rad): {:.4} ({:.2}°)",
            pos,
            pos.to_degrees()
        ),
        Err(e) => info!("Read position error: {}", e),
    }

    match bus.read_raw_position(args.id).await {
        Ok(raw) => info!("Current Position (raw): {}", raw),
        Err(e) => info!("Read raw position error: {}", e),
    }

    let ids = vec![args.id];
    match bus.sync_read_raw_positions(&ids).await {
        Ok(raws) => info!("Sync Current Position (raw): {:?}", raws),
        Err(e) => info!("Sync read raw error: {}", e),
    }

    Ok(())
}
