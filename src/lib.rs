pub mod error;
pub mod bus;
pub mod protocol;
pub mod driver;

// Re-export common types
pub use bus::{MotorBus, ControlOp};
pub use driver::FeetechBus;
pub use error::ServoError;