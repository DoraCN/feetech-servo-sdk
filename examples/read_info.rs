use clap::Parser;
use feetech_servo_sdk::{FeetechBus, MotorBus};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

const ADDR_FIRMWARE_H: u8 = 0x00;
const ADDR_FIRMWARE_L: u8 = 0x01;
const ADDR_SOFTWARE_H: u8 = 0x03;
const ADDR_SOFTWARE_L: u8 = 0x04;
const ADDR_ID: u8 = 0x05;
const ADDR_BAUDRATE: u8 = 0x06;
const ADDR_MODEL_H: u8 = 0x38;
const ADDR_MODEL_L: u8 = 0x39;

#[derive(Parser, Debug)]
#[command(author, version, about = "Read servo information")]
struct Args {
    /// Serial port path (e.g., /dev/ttyUSB0)
    #[arg(short, long, default_value = "/dev/ttyUSB0")]
    port: String,

    /// Baud rate
    #[arg(short, long, default_value_t = 1000000)]
    baud: u32,

    /// Servo ID to read
    #[arg(short, long, default_value_t = 1)]
    id: u8,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let args = Args::parse();

    let mut bus = FeetechBus::new(&args.port, args.baud)?;
    info!("Reading servo info for ID {}...", args.id);

    let firmware_h = bus.read_byte(args.id, ADDR_FIRMWARE_H).await?;
    let firmware_l = bus.read_byte(args.id, ADDR_FIRMWARE_L).await?;
    info!("Firmware Version: {}.{}", firmware_h, firmware_l);

    let software_h = bus.read_byte(args.id, ADDR_SOFTWARE_H).await?;
    let software_l = bus.read_byte(args.id, ADDR_SOFTWARE_L).await?;
    info!("Software Version: {}.{}", software_h, software_l);

    let id = bus.read_byte(args.id, ADDR_ID).await?;
    info!("ID: {}", id);

    let baudrate = bus.read_byte(args.id, ADDR_BAUDRATE).await?;
    let baudrate_str = match baudrate {
        0 => "1,000,000",
        1 => "500,000",
        2 => "250,000",
        3 => "128,000",
        4 => "115,200",
        5 => "76,800",
        6 => "57,600",
        7 => "38,400",
        _ => "Unknown",
    };
    info!("Baud Rate: {}", baudrate_str);

    let model_h = bus.read_byte(args.id, ADDR_MODEL_H).await?;
    let model_l = bus.read_byte(args.id, ADDR_MODEL_L).await?;
    let model = (model_h as u16) << 8 | (model_l as u16);
    info!("Model: {:#06x} ({})", model, model);

    let pos = bus.read_position(args.id).await?;
    info!("Current Position: {:.4} rad ({:.2}°)", pos, pos.to_degrees());

    let raw = bus.read_raw_position(args.id).await?;
    info!("Raw Position: {}", raw);

    info!("Done.");
    Ok(())
}