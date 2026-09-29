//! Bluetooth HCI commands and events for the STM32WB wireless coprocessor.
//!
//! Every declaration is checked at compile time against a catalog extracted
//! from ST's STM32CubeWB sources, for the wireless binary the features
//! select: exactly one `fw_*` feature, the STM32CubeWB release, and exactly
//! one `stack-*` feature, the BLE stack profile. A command or event that
//! binary does not implement is not compiled.
//!
//! The STM32WBA BLE stack is selected the same way, by a `wba_*` release
//! and one of its `stack-wba-*` profiles. No declaration is checked against
//! its catalog yet, so only [`wire`] is compiled for it.
//!
//! - [`aci`] declares the ST vendor commands and events, and [`AciEvent`]
//!   decodes any vendor event. [`aci::status`] names the BLE stack's own
//!   status codes, which bt-hci shows as unknown.
//! - [`event`] decodes any event the BLE stack sends, Core or vendor.
//! - [`standard`] checks bt-hci's Bluetooth Core commands and events against
//!   the catalog, and declares the ones bt-hci gets wrong or lacks.
//! - [`shci`] declares the system commands and events of the CPU2 system
//!   channel.
//! - [`adv_data`] builds advertising and scan response data.
//! - [`wire`] holds the traits and buffers the declarations are built on.
//!
//! # Controller model
//!
//! A transport implements [`bt_hci::controller::Controller`] to read and
//! write packets, and [`bt_hci::controller::ControllerCmdSync`] and
//! [`bt_hci::controller::ControllerCmdAsync`] to execute commands. Bounding
//! those impls with [`catalog::Supported`] restricts them to the commands the
//! selected binary implements, as the catalog describes them. System
//! commands go through the system channel as [`wire::SystemCommand`]s.
//!
//! [`AciEvent`]: aci::AciEvent

#![no_std]
#![allow(async_fn_in_trait)]

// Lets catalog-derived declarations name this crate the same way inside and outside it.
extern crate self as stm32wb_hci;

stm32wb_hci_macros::check_target!();

// This must go FIRST so that all the other modules see its macros.
mod fmt;

#[cfg(feature = "_stm32wb")]
pub mod aci;
#[cfg(feature = "_stm32wb")]
pub mod adv_data;
#[cfg(feature = "_stm32wb")]
#[doc(hidden)]
pub mod catalog;
#[cfg(feature = "_stm32wb")]
pub mod event;
#[cfg(feature = "_stm32wb")]
pub mod shci;
#[cfg(feature = "_stm32wb")]
pub mod standard;
pub mod wire;
