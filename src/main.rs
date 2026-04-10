use anyhow::Result;
use clap::{Parser, Subcommand};
use feetech_servo_sdk::{ControlOp, MotorBus};
use std::time::Duration;
use tracing::{Level, error, info};
use tracing_subscriber::FmtSubscriber;

#[derive(Parser)]
#[command(author, version, about = "Feetech Servo SDK CLI Tool")]
struct Cli {
    /// Serial port (e.g., /dev/ttyUSB0 or COM3). Ignored if --mock is set.
    #[arg(short, long, default_value = "/dev/ttyUSB0")]
    port: String,

    /// Baud rate
    #[arg(short, long, default_value_t = 1000000)]
    baud: u32,

    /// Use Mock backend instead of real hardware
    #[arg(long)]
    mock: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Scan for connected servos (Ping ID 1-253)
    Scan {
        #[arg(long, default_value_t = 20)]
        max_id: u8,
    },
    /// Continuously monitor positions of specific IDs
    Monitor {
        /// Servo IDs to monitor (comma separated, e.g., "1,2,3")
        #[arg(short, long, value_delimiter = ',')]
        ids: Vec<u8>,
    },
    /// Move a servo to a specific position
    Move {
        /// Servo ID
        #[arg(short, long)]
        id: u8,
        /// Target position in DEGREES (0-360)
        #[arg(short, long)]
        degrees: f32,
        /// Execution time in seconds (optional wait)
        #[arg(short, long, default_value_t = 0.0)]
        time: f32,
    },
    /// Reset/Relax (Disable Torque)
    Relax {
        #[arg(short, long, value_delimiter = ',')]
        ids: Vec<u8>,
    },
}

async fn run(mut bus: Box<dyn MotorBus>, cli: Cli) -> Result<()> {
    match &cli.command {
        Commands::Scan { max_id } => {
            info!("Starting scan up to ID {}...", max_id);
            let mut found = Vec::new();
            for id in 1..=*max_id {
                if let Ok(pos) = bus.read_position(id).await {
                    info!("Found ID {}: {:.2}° ({:.4} rad)", id, pos.to_degrees(), pos);
                    found.push(id);
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            info!("Scan complete. Found {} devices: {:?}", found.len(), found);
        }

        Commands::Monitor { ids } => {
            if ids.is_empty() {
                error!("Please provide at least one ID to monitor");
                return Ok(());
            }
            info!("Monitoring IDs: {:?}. Press Ctrl+C to stop.", ids);
            loop {
                match bus.sync_read_positions(ids).await {
                    Ok(positions) => {
                        let displays: Vec<String> = positions
                            .iter()
                            .zip(ids.iter())
                            .map(|(pos, id): (&f32, &u8)| {
                                format!("ID{}: {:.1}°", id, pos.to_degrees())
                            })
                            .collect();
                        info!("Positions: {}", displays.join(" | "));
                    }
                    Err(e) => error!("Read error: {}", e),
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }

        Commands::Move { id, degrees, time } => {
            let rad = degrees.to_radians();
            info!("Moving ID {} to {:.2}° ({:.4} rad)...", id, degrees, rad);
            bus.enable_torque(&[*id]).await?;
            bus.write_goal(*id, ControlOp::Position(rad)).await?;
            info!("Command sent.");
            if *time > 0.0 {
                tokio::time::sleep(Duration::from_secs_f32(*time)).await;
            }
        }

        Commands::Relax { ids } => {
            info!("Disabling torque for IDs: {:?}", ids);
            bus.disable_torque(ids).await?;
            info!("Done.");
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let cli = Cli::parse();

    if cli.mock {
        info!("Running in MOCK mode (Simulating IDs 1-6)");
        #[cfg(feature = "mock")]
        {
            use feetech_servo_sdk::MockBus;
            return run(Box::new(MockBus::new(&[1, 2, 3, 4, 5, 6])), cli).await;
        }
        #[cfg(not(feature = "mock"))]
        anyhow::bail!("Mock mode requires the `mock` feature. Rebuild with --features mock.");
    } else {
        #[cfg(feature = "tokio-serial-impl")]
        {
            use feetech_servo_sdk::FeetechBus;
            info!("Opening serial port {} at {}...", cli.port, cli.baud);
            return run(Box::new(FeetechBus::new(&cli.port, cli.baud)?), cli).await;
        }
        #[cfg(not(feature = "tokio-serial-impl"))]
        anyhow::bail!(
            "Serial port support requires the `tokio-serial-impl` feature (enabled by default)."
        );
    }
}
