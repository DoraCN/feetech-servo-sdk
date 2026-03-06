use clap::Parser;
use feetech_servo_sdk::{ControlOp, FeetechBus, MotorBus};
use std::time::Duration;
use tracing::{Level, error, info};
use tracing_subscriber::FmtSubscriber;

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "Direct PWM (RawEffort) control example for Feetech Servos"
)]
struct Args {
    /// Serial port path
    #[arg(short, long, default_value = "/dev/ttyUSB0")]
    port: String,

    /// Baud rate
    #[arg(short, long, default_value_t = 1000000)]
    baud: u32,

    /// Servo IDs to control (comma separated, e.g., "1,2,3")
    #[arg(long, value_delimiter = ',', default_value = "1,2,3,4,5,6")]
    ids: Vec<u8>,

    /// Raw effort (PWM) value to apply (-4096 to 4096)
    #[arg(short, long, default_value_t = 200)]
    effort: i16,

    /// Duration in seconds to run the motors
    #[arg(short, long, default_value_t = 2.0)]
    duration: f32,

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
        info!("Connecting to {} at {} bps...", args.port, args.baud);
        Box::new(FeetechBus::new(&args.port, args.baud)?)
    };

    info!("Enabling torque for IDs: {:?}", args.ids);
    bus.enable_torque(&args.ids).await?;

    info!(
        "Setting RawEffort (Mapped to Goal Position) to {} for {} seconds...",
        args.effort, args.duration
    );

    for &id in &args.ids {
        if let Err(e) = bus.write_goal(id, ControlOp::RawEffort(args.effort)).await {
            error!("❌ Failed to write RawEffort to ID {}: {}", id, e);
        }
    }

    tokio::time::sleep(Duration::from_secs_f32(args.duration)).await;

    info!("Stopping motors...");
    // Commented out to prevent returning to 0 automatically if desired,
    // but kept as safety fallback depending on user pref.
    /*
    for &id in &args.ids {
        let _ = bus.write_goal(id, ControlOp::RawEffort(0)).await;
    }
    */

    // Optional: disable torque after stopping
    bus.disable_torque(&args.ids).await?;

    info!("Command complete.");

    Ok(())
}
