//! Bluetooth HCI commands and events for STM32WB controllers.
//!
//! This crate provides declarative ST vendor-specific commands, events, and wire types for the
//! STM32WB wireless coprocessor. Standard Bluetooth HCI commands, events, packet framing, and
//! controller traits are available through the public [`bt_hci`] re-export.
//!
//! # Controller model
//!
//! New controller adapters should implement [`bt_hci::controller::Controller`] to read and write
//! transport packets. The [`bt-hci`] command helpers then provide
//! [`bt_hci::controller::ControllerCmdSync`] and [`bt_hci::controller::ControllerCmdAsync`]
//! implementations for command execution.
//!
//! Vendor commands are represented by generated types under [`vendor::command`]. Construct the
//! command and execute it through
//! [`bt_hci::cmd::SyncCmd::exec`] for Command Complete commands or
//! [`bt_hci::cmd::AsyncCmd::exec`] for Command Status commands.
//!
//! Decode the payload of a vendor-specific HCI event with
//! [`vendor::event::VendorEvent::new`]. A few standard commands supported by STM32WB but not yet
//! declared by `bt-hci` live in [`standard`].
//!
//! [`Bluetooth`]: https://www.bluetooth.com/specifications/bluetooth-core-specification
//! [`bt-hci`]: https://crates.io/crates/bt-hci

#![no_std]
#![allow(async_fn_in_trait)]

/// The standard Bluetooth HCI command and event implementation used by this
/// crate.
///
/// STM32WB-specific APIs live under [`vendor`].  Re-exporting the underlying
/// standard-HCI crate makes its command/event surface available to callers.
/// Firmware compliance covers only APIs implemented directly by this crate.
pub use bt_hci;

// This must go FIRST so that all the other modules see its macros.
mod fmt;

#[macro_use]
mod wire;

pub mod standard;
pub mod types;
pub mod vendor;
