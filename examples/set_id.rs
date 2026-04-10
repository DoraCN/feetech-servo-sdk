use clap::Parser;
use feetech_servo_sdk::FeetechBus;
use feetech_servo_sdk::FeetechController;
use tracing::{info, error, Level};
use tracing_subscriber::FmtSubscriber;

const ADDR_ID: u8 = 0x05;
const ADDR_LOCK: u8 = 0x37;
const ADDR_BAUDRATE: u8 = 0x06;

#[derive(Parser, Debug)]
#[command(author, version, about = "Set ID for a Feetech Servo")]
struct Args {
    /// Serial port path (e.g., /dev/ttyUSB0)
    #[arg(short, long, default_value = "/dev/ttyUSB0")]
    port: String,

    /// Baud rate
    #[arg(short, long, default_value_t = 1000000)]
    baud: u32,

    /// New ID to set (1-253)
    #[arg(short, long)]
    new_id: u8,

    /// Also set baudrate (0=1M, 1=500K, 2=250K, 3=128K, 4=115200, 5=76800, 6=57600, 7=38400)
    #[arg(long)]
    baudrate_code: Option<u8>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let args = Args::parse();

    if args.new_id == 0xFE {
        error!("ID 0xFE is reserved for broadcast");
        return Ok(());
    }

    info!("Connecting to {}...", args.port);
    let mut bus = FeetechBus::new(&args.port, args.baud)?;

    info!("Using broadcast ID to set ID for connected servo...");
    info!("Setting servo ID to {}...", args.new_id);

    info!("Step 1: Unlock EEPROM (write 0 to address {:#04x})", ADDR_LOCK);
    FeetechController::broadcast_write(&mut bus, ADDR_LOCK, 0).await?;

    info!("Step 2: Write new ID {} to address {:#04x}", args.new_id, ADDR_ID);
    FeetechController::broadcast_write(&mut bus, ADDR_ID, args.new_id).await?;

    info!("Step 3: Lock EEPROM to protect settings");
    FeetechController::broadcast_write(&mut bus, ADDR_LOCK, 1).await?;

    if let Some(br_code) = args.baudrate_code {
        info!("Step 4: Setting baud rate code to {} at address {:#04x}", br_code, ADDR_BAUDRATE);
        FeetechController::broadcast_write(&mut bus, ADDR_LOCK, 0).await?;
        FeetechController::broadcast_write(&mut bus, ADDR_BAUDRATE, br_code).await?;
        FeetechController::broadcast_write(&mut bus, ADDR_LOCK, 1).await?;
    }

    info!("ID set successfully to {}", args.new_id);
    info!("IMPORTANT: Power cycle the servo for changes to take effect!");
    info!("Then verify with: cargo run --example scan -- --port {}", args.port);

    Ok(())
}