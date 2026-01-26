pub mod error;
pub mod bus;
pub mod protocol;
pub mod driver;

// 添加 Mock 模块，并使其公开
// #[cfg(feature = "mock")] // 可选：只在开启 mock feature 时编译
pub mod mock;

// Re-export common types
pub use bus::{MotorBus, ControlOp};
pub use driver::FeetechBus;
pub use error::ServoError;
// pub use mock::MockBus; // 可选导出