//! Parsers for ST-generated STM32CubeWB C sources and headers.
//!
//! The parser is intentionally narrow: it recognizes the generated formatting
//! used by the files under `Middlewares/ST/STM32_WPAN/ble/core/auto`. The public
//! functions assemble those narrow parsers into firmware command and packed
//! struct specs consumed by the compliance checker.

mod common;
mod docs;
mod events;
mod function;
mod packed_struct;
mod payload;
mod signature;

use crate::spec::{CommandSpec, PackedStructSpec};
use anyhow::Result;
use std::collections::HashMap;

/// Parses one ST command group from its generated `.c` and `.h` files.
///
/// Only `aci_*` functions are currently included. Standard `hci_*` functions
/// are skipped by design until the Rust checker grows matching HCI coverage.
pub fn parse_group(source_name: &str, source: &str, header: &str) -> Result<Vec<CommandSpec>> {
    let docs = docs::parse_command_docs(header)?;
    let mut commands = Vec::new();

    for function in function::split_functions(source)? {
        let Some(name) = function
            .name
            .strip_prefix("aci_")
            .map(|_| function.name.clone())
        else {
            continue;
        };

        let doc = docs.get(&name);
        let params = signature::parse_signature_params(&function.signature, doc)?;
        let param_types = params
            .iter()
            .map(|p| (p.name.clone(), p.c_type.clone()))
            .collect::<HashMap<_, _>>();

        let ogf = function::parse_hex_assignment(&function.body, "ogf")?;
        let ocf = function::parse_hex_assignment(&function.body, "ocf")?;
        let opcode = match (ogf, ocf) {
            (Some(ogf), Some(ocf)) => Some((ogf << 10) | ocf),
            _ => None,
        };

        commands.push(CommandSpec {
            group: source_name.to_owned(),
            name,
            ogf,
            ocf,
            opcode,
            event: function::parse_hex_assignment(&function.body, "event")?.map(|v| v as u8),
            return_len: function::parse_decimal_assignment(&function.body, "rlen")?,
            doc: doc.map(|d| d.command.clone()),
            payload: payload::parse_payload(&function.body, &param_types, doc)?,
            params,
        });
    }

    Ok(commands)
}

/// Parses packed structs from ST's generated `ble_types.h`.
pub fn parse_packed_structs(source: &str) -> Result<Vec<PackedStructSpec>> {
    packed_struct::parse_packed_structs(source)
}

/// Parses generated vendor event prototypes from ST's `ble_events.h`.
pub fn parse_events(header: &str) -> Vec<crate::spec::EventSpec> {
    events::parse_events(header)
}
