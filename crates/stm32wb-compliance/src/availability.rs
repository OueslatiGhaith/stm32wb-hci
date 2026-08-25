//! Resolve the shared generated BLE interface into one CPU2 binary profile.
//!
//! The generated C wrappers describe the complete host-side protocol catalog;
//! they contain no family or binary membership guards. Cube's tagged Wireless
//! Interface HTML supplies the BF/PO/LO/LB/BO availability matrix, while each
//! family's binary release notes map those columns to exact `.bin` files.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;

use quick_xml::Reader;
use quick_xml::events::Event;

use crate::catalog::{CatalogSchema, CommandScope, EventScope};
use crate::target::{ComplianceTarget, StackProfile};

const INTERFACE_DOCUMENT: &str =
    "Middlewares/ST/STM32_WPAN/ble/core/doc/STM32WB_BLE_Wireless_Interface.html";

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum AvailabilityKey {
    Command(CommandScope, u16),
    Event(EventScope, u16),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AvailabilityEntry {
    name: String,
    reduced_profiles: BTreeSet<StackProfile>,
}

impl AvailabilityEntry {
    fn supports(&self, profile: StackProfile) -> bool {
        profile == StackProfile::FullExtended || self.reduced_profiles.contains(&profile)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TableKind {
    Commands,
    StandardEvents,
    LeMetaEvents,
    VendorEvents,
}

/// Verify the target's tagged binary evidence and filter the complete C
/// catalog using the matching tagged Wireless Interface availability matrix.
pub(crate) fn resolve_target_catalog(
    cube_dir: &Path,
    target: ComplianceTarget,
    catalog: &mut CatalogSchema,
) -> Result<(), String> {
    verify_binary_mapping(cube_dir, target)?;
    verify_binary_blob(cube_dir, target)?;

    let interface = git_show_text(cube_dir, &target.release.cube_tag(), INTERFACE_DOCUMENT)?;
    let availability = parse_interface_availability(&interface)?;

    let mut unavailable_commands = Vec::new();
    for command in &catalog.commands {
        let key = AvailabilityKey::Command(command.scope(), command.code());
        let entry = availability.get(&key).ok_or_else(|| {
            format!(
                "{}: generated command {} ({:?} 0x{:04X}) has no profile-availability row",
                INTERFACE_DOCUMENT,
                command.name,
                command.scope(),
                command.code()
            )
        })?;
        validate_documented_name(&command.name, entry, "command", command.code())?;
        if !entry.supports(target.profile) {
            unavailable_commands.push((command.scope(), command.code()));
        }
    }

    let mut unavailable_events = Vec::new();
    for event in &catalog.events {
        // SHCI is a system interface shared by BLE CPU2 binaries and is
        // catalogued from shci.h, not the BLE Wireless Interface document.
        if event.scope() == EventScope::SystemShci {
            continue;
        }
        let key = AvailabilityKey::Event(event.scope(), event.code);
        let entry = availability.get(&key).ok_or_else(|| {
            format!(
                "{}: generated event {} ({:?} 0x{:04X}) has no profile-availability row",
                INTERFACE_DOCUMENT,
                event.name,
                event.scope(),
                event.code
            )
        })?;
        validate_documented_name(&event.name, entry, "event", event.code)?;
        if !entry.supports(target.profile) {
            unavailable_events.push((event.scope(), event.code));
        }
    }

    catalog
        .commands
        .retain(|command| !unavailable_commands.contains(&(command.scope(), command.code())));
    catalog
        .events
        .retain(|event| !unavailable_events.contains(&(event.scope(), event.code)));
    catalog.normalize()
}

fn validate_documented_name(
    generated: &str,
    documented: &AvailabilityEntry,
    kind: &str,
    code: u16,
) -> Result<(), String> {
    if normalized_identifier(generated) == normalized_identifier(&documented.name) {
        Ok(())
    } else {
        Err(format!(
            "{INTERFACE_DOCUMENT}: {kind} 0x{code:04X} is named {} in generated C but {} in the availability table",
            generated, documented.name
        ))
    }
}

fn normalized_identifier(value: &str) -> String {
    let value = value.strip_suffix("_process").unwrap_or(value);
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_uppercase)
        .collect()
}

fn verify_binary_mapping(cube_dir: &Path, target: ComplianceTarget) -> Result<(), String> {
    let path = target.release_notes_path();
    let path = path
        .to_str()
        .ok_or_else(|| format!("non-UTF-8 release-notes path: {}", path.display()))?;
    let notes = git_show_text(cube_dir, &target.release.cube_tag(), path)?;
    let tables = parse_html_tables(&notes, path)?;
    let expected_binary = target.binary_file_name();
    let mut matches = Vec::new();

    for table in tables {
        let Some(header) = table.first() else {
            continue;
        };
        if normalized_cells(header)
            != [
                "Wireless Coprocessor Binary",
                "stack features naming (3)",
                "#define used in FW M0 code",
            ]
        {
            continue;
        }
        for row in table.iter().skip(1).filter(|row| row.len() == 3) {
            if row[0] != expected_binary {
                continue;
            }
            let profile = profile_from_release_notes_row(row).ok_or_else(|| {
                format!(
                    "{path}: could not map binary {} from stack feature cell {:?}",
                    row[0], row[1]
                )
            })?;
            matches.push(profile);
        }
    }

    match matches.as_slice() {
        [profile] if *profile == target.profile => Ok(()),
        [profile] => Err(format!(
            "{path}: binary {expected_binary} maps to profile {profile}, not {}",
            target.profile
        )),
        [] => Err(format!(
            "{path}: binary {expected_binary} is absent from the wireless-binary/profile mapping"
        )),
        _ => Err(format!(
            "{path}: binary {expected_binary} appears more than once in the wireless-binary/profile mapping"
        )),
    }
}

fn profile_from_release_notes_row(row: &[String]) -> Option<StackProfile> {
    let feature = row.get(1)?.trim();
    if feature == "-" {
        return Some(StackProfile::FullExtended);
    }
    StackProfile::from_documentation_column(feature.split_whitespace().next()?)
}

fn verify_binary_blob(cube_dir: &Path, target: ComplianceTarget) -> Result<(), String> {
    let spec = format!(
        "{}:{}",
        target.release.cube_tag(),
        target.binary_path().display()
    );
    let output = Command::new("git")
        .arg("-C")
        .arg(cube_dir)
        .args(["cat-file", "-e", &spec])
        .output()
        .map_err(|error| format!("could not inspect target binary {spec}: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "target binary {spec} was not found: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

fn parse_interface_availability(
    source: &str,
) -> Result<BTreeMap<AvailabilityKey, AvailabilityEntry>, String> {
    let tables = parse_html_tables(source, INTERFACE_DOCUMENT)?;
    let mut availability = BTreeMap::new();
    let mut recognized_tables = 0_usize;

    for table in tables {
        let Some(header) = table.first() else {
            continue;
        };
        let Some(kind) = table_kind(header) else {
            continue;
        };
        recognized_tables += 1;
        let profile_columns = profile_columns(header)?;
        for row in table.iter().skip(1) {
            if row.len() != header.len() {
                return Err(format!(
                    "{INTERFACE_DOCUMENT}: availability row has {} cells but its header has {}: {row:?}",
                    row.len(),
                    header.len()
                ));
            }
            let name = row[0].trim().to_owned();
            let code = parse_hex_code(&row[1])?;
            let key = availability_key(kind, &name, code)?;
            let mut reduced_profiles = BTreeSet::new();
            for (index, profile) in &profile_columns {
                match row[*index].trim() {
                    "Y" => {
                        reduced_profiles.insert(*profile);
                    }
                    "" => {}
                    value => {
                        return Err(format!(
                            "{INTERFACE_DOCUMENT}: {name} has unsupported availability marker {value:?} in column {}",
                            header[*index]
                        ));
                    }
                }
            }
            let entry = AvailabilityEntry {
                name,
                reduced_profiles,
            };
            if let Some(previous) = availability.insert(key.clone(), entry) {
                return Err(format!(
                    "{INTERFACE_DOCUMENT}: duplicate availability key {key:?} for {} and {}",
                    previous.name, availability[&key].name
                ));
            }
        }
    }

    if recognized_tables == 0 {
        return Err(format!(
            "{INTERFACE_DOCUMENT}: no command or event availability tables were found"
        ));
    }
    Ok(availability)
}

fn table_kind(header: &[String]) -> Option<TableKind> {
    match header.first()?.trim() {
        "Command" if header.get(1)?.trim() == "Opcode" => Some(TableKind::Commands),
        "Event name" if header.get(1)?.trim() == "Event code" => Some(TableKind::StandardEvents),
        "Event name" if header.get(1)?.trim() == "LE subevent code" => {
            Some(TableKind::LeMetaEvents)
        }
        "Event name" if header.get(1)?.trim() == "Vendor specific subevent code" => {
            Some(TableKind::VendorEvents)
        }
        _ => None,
    }
}

fn profile_columns(header: &[String]) -> Result<Vec<(usize, StackProfile)>, String> {
    let mut columns = Vec::new();
    for (index, value) in header.iter().enumerate().skip(2) {
        let profile = StackProfile::from_documentation_column(value).ok_or_else(|| {
            format!("{INTERFACE_DOCUMENT}: unknown availability column {value:?} in {header:?}")
        })?;
        columns.push((index, profile));
    }
    if columns.is_empty() {
        return Err(format!(
            "{INTERFACE_DOCUMENT}: availability table has no reduced-profile columns: {header:?}"
        ));
    }
    Ok(columns)
}

fn availability_key(kind: TableKind, name: &str, code: u16) -> Result<AvailabilityKey, String> {
    match kind {
        TableKind::Commands => {
            if code >> 10 == 0x3f {
                Ok(AvailabilityKey::Command(
                    CommandScope::VendorAci,
                    code & 0x03ff,
                ))
            } else {
                Ok(AvailabilityKey::Command(CommandScope::StandardHci, code))
            }
        }
        TableKind::StandardEvents => Ok(AvailabilityKey::Event(EventScope::StandardHci, code)),
        // Cube 1.24 labels the ACI General events table "LE subevent code"
        // even though those rows are carried in the vendor-specific event
        // table. The ACI prefix is the stable namespace discriminator.
        TableKind::LeMetaEvents if name.starts_with("ACI_") => {
            Ok(AvailabilityKey::Event(EventScope::VendorAci, code))
        }
        TableKind::LeMetaEvents => Ok(AvailabilityKey::Event(EventScope::LeMeta, code)),
        TableKind::VendorEvents => Ok(AvailabilityKey::Event(EventScope::VendorAci, code)),
    }
}

fn parse_hex_code(value: &str) -> Result<u16, String> {
    let value = value.trim();
    let digits = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .ok_or_else(|| format!("{INTERFACE_DOCUMENT}: expected hexadecimal code, got {value:?}"))?;
    u16::from_str_radix(digits, 16)
        .map_err(|_| format!("{INTERFACE_DOCUMENT}: invalid hexadecimal code {value:?}"))
}

fn git_show_text(cube_dir: &Path, tag: &str, path: &str) -> Result<String, String> {
    let spec = format!("{tag}:{path}");
    let output = Command::new("git")
        .arg("-C")
        .arg(cube_dir)
        .arg("show")
        .arg(&spec)
        .output()
        .map_err(|error| format!("could not run git show {spec}: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "git show {spec} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    String::from_utf8(output.stdout)
        .map_err(|error| format!("git show {spec} did not return UTF-8 HTML: {error}"))
}

fn normalized_cells(cells: &[String]) -> Vec<&str> {
    cells.iter().map(|cell| cell.trim()).collect()
}

/// Extract table rows from Cube's generated HTML. quick-xml is used as a
/// streaming tokenizer with HTML-tolerant end-tag settings; the document does
/// not need to be well-formed XML for table cell boundaries to remain exact.
fn parse_html_tables(source: &str, source_name: &str) -> Result<Vec<Vec<Vec<String>>>, String> {
    let mut reader = Reader::from_str(source);
    reader.config_mut().check_end_names = false;
    reader.config_mut().allow_unmatched_ends = true;

    let mut tables = Vec::new();
    let mut table: Option<Vec<Vec<String>>> = None;
    let mut row: Option<Vec<String>> = None;
    let mut cell: Option<String> = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(tag)) => match tag.local_name().as_ref() {
                b"table" => table = Some(Vec::new()),
                b"tr" if table.is_some() => row = Some(Vec::new()),
                b"th" | b"td" if row.is_some() => cell = Some(String::new()),
                _ => {}
            },
            Ok(Event::Text(text)) => {
                if let Some(cell) = cell.as_mut() {
                    let value = text.html_content().map_err(|error| {
                        format!("{source_name}: could not decode HTML text: {error}")
                    })?;
                    cell.push_str(&value);
                }
            }
            Ok(Event::CData(text)) => {
                if let Some(cell) = cell.as_mut() {
                    let value = text.html_content().map_err(|error| {
                        format!("{source_name}: could not decode HTML CDATA: {error}")
                    })?;
                    cell.push_str(&value);
                }
            }
            Ok(Event::End(tag)) => match tag.local_name().as_ref() {
                b"th" | b"td" => {
                    if let (Some(row), Some(value)) = (row.as_mut(), cell.take()) {
                        row.push(normalize_html_text(&value));
                    }
                }
                b"tr" => {
                    if let (Some(table), Some(row)) = (table.as_mut(), row.take())
                        && !row.is_empty()
                    {
                        table.push(row);
                    }
                }
                b"table" => {
                    if let Some(table) = table.take()
                        && !table.is_empty()
                    {
                        tables.push(table);
                    }
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => {
                return Err(format!(
                    "{source_name}: invalid generated HTML near byte {}: {error}",
                    reader.error_position()
                ));
            }
        }
    }
    Ok(tables)
}

fn normalize_html_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_profile_columns_for_commands_and_events() {
        let html = r#"
            <table>
              <tr><th>Command</th><th>Opcode</th><th>BF</th><th>PO</th><th>LO</th><th>LB</th><th>BO</th></tr>
              <tr><td><a>ACI_GATT_INIT</a></td><td><p>0xFD01</p></td><td><p>Y</p></td><td>Y</td><td></td><td></td><td></td></tr>
            </table>
            <table>
              <tr><th>Event name</th><th>Vendor specific subevent code</th><th>BF</th><th>PO</th><th>LO</th><th>LB</th><th>BO</th></tr>
              <tr><td>ACI_GAP_PROC_COMPLETE_EVENT</td><td>0x0407</td><td>Y</td><td>Y</td><td></td><td></td><td></td></tr>
            </table>
        "#;
        let values = parse_interface_availability(html).unwrap();
        let command = &values[&AvailabilityKey::Command(CommandScope::VendorAci, 0x101)];
        assert!(command.supports(StackProfile::FullExtended));
        assert!(command.supports(StackProfile::Full));
        assert!(command.supports(StackProfile::Light));
        assert!(!command.supports(StackProfile::HciLayerExtended));
        let event = &values[&AvailabilityKey::Event(EventScope::VendorAci, 0x0407)];
        assert!(event.supports(StackProfile::Light));
        assert!(!event.supports(StackProfile::HciAdvScan));
    }

    #[test]
    fn parses_family_release_note_binary_mapping() {
        let html = r#"
            <table><tr><th>Wireless Coprocessor Binary</th><th>stack features naming (3)</th><th>#define used in FW M0 code</th></tr>
            <tr><td>stm32wb5x_BLE_Stack_full_extended_fw.bin</td><td>-</td><td>- (1)</td></tr>
            <tr><td>stm32wb5x_BLE_Stack_light_fw.bin</td><td>PO = “Peripheral Only”</td><td>SLAVE_ONLY</td></tr></table>
        "#;
        let tables = parse_html_tables(html, "fixture").unwrap();
        assert_eq!(
            profile_from_release_notes_row(&tables[0][1]),
            Some(StackProfile::FullExtended)
        );
        assert_eq!(
            profile_from_release_notes_row(&tables[0][2]),
            Some(StackProfile::Light)
        );
    }

    #[test]
    fn rejects_unknown_availability_markers() {
        let html = r#"
            <table><tr><th>Command</th><th>Opcode</th><th>BF</th></tr>
            <tr><td>ACI_RESET</td><td>0xFF00</td><td>maybe</td></tr></table>
        "#;
        let error = parse_interface_availability(html).unwrap_err();
        assert!(error.contains("unsupported availability marker"));
    }
}
