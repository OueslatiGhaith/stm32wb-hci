//! Completeness of the declarations against the catalog.
//!
//! Every vendor command and event the selected target's wireless binary
//! implements must be declared in [`crate::aci`]; a missing one fails to
//! compile here, naming it.

stm32wb_hci_macros::catalog_complete!();
