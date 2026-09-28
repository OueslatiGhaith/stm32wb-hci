//! Completeness of the declarations against the catalog, and the commands
//! the selected target supports.
//!
//! Every vendor command and event the selected target's wireless binary
//! implements must be declared in [`crate::aci`], and every system command
//! and event in [`crate::shci`]; a missing one fails to compile here, naming
//! it.

/// A bt-hci command the selected target's wireless binary implements, with
/// the opcode, completion, and parameter layout the catalog lists for it.
///
/// Implemented by every vendor command [`crate::aci`] declares, and by the
/// bt-hci standard commands [`crate::standard`] checks against the catalog,
/// on the targets whose binary implements them. A transport can bound its
/// commands with it to reject, at compile time, a command the binary would
/// answer with Unknown HCI Command.
#[diagnostic::on_unimplemented(
    message = "the selected target's wireless binary does not implement `{Self}`, or the catalog does not describe it as bt-hci does",
    note = "see stm32wb_hci::standard for the bt-hci commands checked against the catalog"
)]
pub trait Supported: bt_hci::cmd::Cmd {}

stm32wb_hci_macros::catalog_complete!();
