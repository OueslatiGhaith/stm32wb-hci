//! Parser for generated ST event prototypes.
//!
//! Event metadata lives in `ble_events.h` as Doxygen blocks followed by
//! `void <event_name>(...)` prototypes. For coverage we only need the generated
//! C event function name because it maps directly to an ST event name.

use super::common::find_function_prototypes;
use crate::spec::EventSpec;

/// Parses generated event prototypes from `ble_events.h`.
pub(super) fn parse_events(header: &str) -> Vec<EventSpec> {
    find_function_prototypes(header, "void")
        .into_iter()
        .filter(|prototype| {
            prototype.name.starts_with("aci_") && prototype.name.ends_with("_event")
        })
        .map(|prototype| EventSpec {
            name: prototype.name,
        })
        .collect()
}
