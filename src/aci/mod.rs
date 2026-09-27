//! ST vendor (ACI) commands declared against the STM32WB catalog.
//!
//! Each command's opcode, completion kind, and availability come from the
//! catalog for the release and stack profile selected by the `fw_*` and
//! `stack-*` features; a command the selected binary does not implement is
//! not compiled.

pub mod att;
pub mod gap;
pub mod gatt;
pub mod general;
pub mod hal;
pub mod l2cap;
