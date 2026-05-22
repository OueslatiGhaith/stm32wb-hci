//! Parser for generated ST event prototypes.
//!
//! Event metadata lives in `ble_events.h` as Doxygen blocks followed by
//! `void <event_name>(...)` prototypes. For coverage we only need the generated
//! C event function name because it maps directly to an ST event name.

use super::{common::find_function_prototypes, docs, signature};
use crate::spec::EventSpec;
use anyhow::Result;

/// Parses generated event prototypes from `ble_events.h`.
pub(super) fn parse_events(header: &str) -> Result<Vec<EventSpec>> {
    let docs = docs::parse_function_docs(header, "void")?;
    find_function_prototypes(header, "void")
        .into_iter()
        .filter(|prototype| {
            prototype.name.starts_with("aci_") && prototype.name.ends_with("_event")
        })
        .map(|prototype| {
            let doc = docs.get(&prototype.name);
            Ok(EventSpec {
                name: prototype.name,
                doc: doc.map(|d| d.command.clone()),
                params: signature::parse_signature_params(&prototype.signature, doc)?,
            })
        })
        .collect()
}
