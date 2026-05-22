//! Parser for generated ST event prototypes.
//!
//! Event metadata lives in `ble_events.h` as Doxygen blocks followed by
//! `void <event_name>(...)` prototypes. For coverage we only need the generated
//! C event function name because it maps directly to an ST event name.

use crate::spec::EventSpec;

/// Parses generated event prototypes from `ble_events.h`.
pub(super) fn parse_events(header: &str) -> Vec<EventSpec> {
    header
        .lines()
        .filter_map(parse_event_prototype)
        .filter(|name| name.starts_with("aci_") && name.ends_with("_event"))
        .map(|name| EventSpec { name })
        .collect()
}

/// Parses a one-line `void aci_*_event(` prototype prefix.
fn parse_event_prototype(line: &str) -> Option<String> {
    let line = line.trim();
    let rest = line.strip_prefix("void ")?;
    let name = rest.split_once('(')?.0.trim();
    (!name.is_empty()).then(|| name.to_owned())
}
