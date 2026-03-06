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

    #[arg(long, value_delimiter = ',', default_value = "1,2,3,4,5,6")]
    ids: Vec<u8>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let args = Args::parse();

    info!("Connecting to {}...", args.port);
    let mut bus: Box<dyn MotorBus> = Box::new(FeetechBus::new(&args.port, args.baud)?);

    info!("Reading IDs {:?}...", args.ids);

    match bus.sync_read_raw_positions(&args.ids).await {
        Ok(raws) => info!("Sync Current Position (raw): {:?}", raws),
        Err(e) => info!("Sync read raw error: {}", e),
    }

    Ok(())
}
