//! The normalized STM32WB wireless-interface catalog.
//!
//! The catalog is the single description of what each STM32CubeWB release's
//! CPU2 wireless binaries accept and emit. It has three layers, each with a
//! distinct source:
//!
//! 1. **Generated C** (`ble_*_aci.c`, `ble_hci_le.c`, `ble_events.c`,
//!    `ble_types.h`, `shci.h`): opcodes, completion kinds, and wire layouts.
//! 2. **ST documents** (`STM32WB_BLE_Wireless_Interface.html` and each
//!    family's `Release_Notes.html`): which stack profile supports each
//!    command and event, and which binaries exist per MCU family.
//! 3. **Curated annotations**: facts no ST artifact states in a
//!    machine-readable form, each with a cited source.
//!
//! Layers 1 and 2 are written by `stm32wb-catalog-extract` into a checked-in
//! file that can be reproduced from the tagged Cube sources. Layer 3 is a
//! separate hand-maintained file audited against the extracted layers.

mod error;
pub mod layout;
mod release;
mod target;

pub use error::{Error, ErrorKind};
pub use layout::{Element, Envelope, Field, FieldType, Layout, Scalar, Structs, UnionVariant};
pub use release::{ReleaseRange, Version};
pub use target::{Family, Profile};
