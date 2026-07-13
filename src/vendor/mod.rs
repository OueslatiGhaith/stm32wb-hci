//! Vendor specific commands for STM32WB family

pub mod command;
pub mod event;
pub mod opcode;

/// specify vendor specifi extensions for STM32WB family
pub use crate::host::uart::CommandHeader;
pub use event::VendorError;
