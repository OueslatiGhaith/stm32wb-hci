//! Layer 2: facts from ST's tagged HTML documents.
//!
//! - `STM32WB_BLE_Wireless_Interface.html` lists every command and event with
//!   one availability column per reduced stack profile (BF, PO, LO, LB, BO).
//!   The full-extended profile supports the complete interface.
//! - Each family's `Release_Notes.html` maps binary files to stack profiles.

use std::collections::{BTreeMap, BTreeSet};

use quick_xml::Reader;
use quick_xml::events::Event;
use stm32wb_catalog::{CommandScope, EventScope, Family, Profile, SnapshotBinary};

use crate::cube::{BINARIES_DIR, CubeTag, INTERFACE_DOCUMENT};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Key {
    Command(CommandScope, u16),
    Event(EventScope, u16),
}

#[derive(Clone, Debug)]
pub struct Documented {
    pub name: String,
    pub profiles: Vec<Profile>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TableKind {
    Commands,
    StandardEvents,
    LeMetaEvents,
    VendorEvents,
}

pub fn interface_availability(tag: &CubeTag<'_>) -> Result<BTreeMap<Key, Documented>, String> {
    let source = tag.read_text(INTERFACE_DOCUMENT)?;
    parse_interface_availability(&source).map_err(|error| format!("{INTERFACE_DOCUMENT}: {error}"))
}

fn parse_interface_availability(source: &str) -> Result<BTreeMap<Key, Documented>, String> {
    let mut availability = BTreeMap::new();
    let mut recognized = 0usize;
    for table in parse_html_tables(source)? {
        let Some(header) = table.first() else {
            continue;
        };
        let Some(kind) = table_kind(header) else {
            continue;
        };
        recognized += 1;
        let mut columns = Vec::new();
        for (index, heading) in header.iter().enumerate().skip(2) {
            let profile = Profile::from_documentation_column(heading)
                .ok_or_else(|| format!("unknown availability column {heading:?}"))?;
            columns.push((index, profile));
        }
        if columns.is_empty() {
            return Err(format!(
                "availability table without profile columns: {header:?}"
            ));
        }
        for row in table.iter().skip(1) {
            if row.len() != header.len() {
                return Err(format!(
                    "row has {} cells but its header has {}: {row:?}",
                    row.len(),
                    header.len()
                ));
            }
            let name = row[0].clone();
            let code = parse_hex(&row[1])?;
            let key = match kind {
                TableKind::Commands if code >> 10 == 0x3F => {
                    Key::Command(CommandScope::Vendor, code)
                }
                TableKind::Commands => Key::Command(CommandScope::Standard, code),
                TableKind::StandardEvents => Key::Event(EventScope::Standard, code),
                // Some releases file the ACI General events under an "LE
                // subevent code" heading although Cube dispatches them from
                // the vendor table; the ACI_ namespace is authoritative.
                TableKind::LeMetaEvents if name.starts_with("ACI_") => {
                    Key::Event(EventScope::Vendor, code)
                }
                TableKind::LeMetaEvents => Key::Event(EventScope::LeMeta, code),
                TableKind::VendorEvents => Key::Event(EventScope::Vendor, code),
            };
            let mut profiles = BTreeSet::from([Profile::FullExtended]);
            for (index, profile) in &columns {
                match row[*index].as_str() {
                    "Y" => {
                        profiles.insert(*profile);
                    }
                    "" => {}
                    other => {
                        return Err(format!("{name}: unknown availability marker {other:?}"));
                    }
                }
            }
            let entry = Documented {
                name,
                profiles: profiles.into_iter().collect(),
            };
            if let Some(previous) = availability.insert(key.clone(), entry) {
                return Err(format!("{key:?} is documented twice ({})", previous.name));
            }
        }
    }
    if recognized == 0 {
        return Err("no availability tables were found".to_owned());
    }
    Ok(availability)
}

fn table_kind(header: &[String]) -> Option<TableKind> {
    match (header.first()?.as_str(), header.get(1)?.as_str()) {
        ("Command", "Opcode") => Some(TableKind::Commands),
        ("Event name", "Event code") => Some(TableKind::StandardEvents),
        ("Event name", "LE subevent code") => Some(TableKind::LeMetaEvents),
        ("Event name", "Vendor specific subevent code") => Some(TableKind::VendorEvents),
        _ => None,
    }
}

fn parse_hex(value: &str) -> Result<u16, String> {
    value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .and_then(|digits| u16::from_str_radix(digits, 16).ok())
        .ok_or_else(|| format!("invalid hexadecimal code {value:?}"))
}

/// Whether a generated C name and a documented name denote the same entry.
pub fn same_name(generated: &str, documented: &str) -> bool {
    let normalize = |value: &str| {
        value
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .flat_map(|character| character.to_uppercase())
            .collect::<String>()
    };
    normalize(generated) == normalize(documented)
}

/// Binaries each family's release notes map to a BLE stack profile, each
/// verified to exist at the tag.
pub fn binaries(tag: &CubeTag<'_>) -> Result<Vec<SnapshotBinary>, String> {
    let mut binaries = Vec::new();
    for family in Family::ALL {
        let directory = format!("{BINARIES_DIR}/{}", family.directory());
        let files = tag.list(&directory)?;
        if files.is_empty() {
            continue;
        }
        let notes_path = format!("{directory}/Release_Notes.html");
        let notes = tag.read_text(&notes_path)?;
        let mapping =
            parse_release_notes(&notes).map_err(|error| format!("{notes_path}: {error}"))?;
        for profile in Profile::ALL {
            let file = family.binary_file_name(profile);
            let Some(documented) = mapping.get(&file) else {
                continue;
            };
            if *documented != profile {
                return Err(format!(
                    "{notes_path}: {file} is documented as {documented}"
                ));
            }
            if !files.contains(&format!("{directory}/{file}")) {
                return Err(format!(
                    "{notes_path} lists {file}, which is not in the tag"
                ));
            }
            binaries.push(SnapshotBinary { family, profile });
        }
    }
    Ok(binaries)
}

/// File name -> documented profile, for rows naming a BLE stack profile.
fn parse_release_notes(source: &str) -> Result<BTreeMap<String, Profile>, String> {
    let mut mapping = BTreeMap::new();
    let mut found = false;
    for table in parse_html_tables(source)? {
        let Some(header) = table.first() else {
            continue;
        };
        if header.len() != 3
            || header[0] != "Wireless Coprocessor Binary"
            || !header[1].starts_with("stack features naming")
        {
            continue;
        }
        found = true;
        for row in table.iter().skip(1) {
            if row.len() != 3 {
                return Err(format!("a binary row has {} cells: {row:?}", row.len()));
            }
            // A binary the catalog cannot name the profile of would drop out
            // of it unnoticed.
            let profile = if row[1] == "-" {
                Profile::FullExtended
            } else {
                row[1]
                    .split_whitespace()
                    .next()
                    .and_then(Profile::from_documentation_column)
                    .ok_or_else(|| {
                        format!("{} has an unknown stack profile {:?}", row[0], row[1])
                    })?
            };
            if mapping.insert(row[0].clone(), profile).is_some() {
                return Err(format!("{} is listed twice", row[0]));
            }
        }
    }
    if found {
        Ok(mapping)
    } else {
        Err("no binary/stack-feature table was found".to_owned())
    }
}

/// Table cells from Cube's generated HTML, with whitespace collapsed.
/// quick-xml tokenizes with HTML-tolerant end tags; the documents need not
/// be well-formed XML for cell boundaries to stay exact.
fn parse_html_tables(source: &str) -> Result<Vec<Vec<Vec<String>>>, String> {
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
                    cell.push_str(
                        &text
                            .html_content()
                            .map_err(|error| format!("could not decode HTML text: {error}"))?,
                    );
                }
            }
            Ok(Event::CData(text)) => {
                if let Some(cell) = cell.as_mut() {
                    cell.push_str(
                        &text
                            .html_content()
                            .map_err(|error| format!("could not decode HTML CDATA: {error}"))?,
                    );
                }
            }
            Ok(Event::End(tag)) => match tag.local_name().as_ref() {
                b"th" | b"td" => {
                    if let (Some(row), Some(value)) = (row.as_mut(), cell.take()) {
                        row.push(value.split_whitespace().collect::<Vec<_>>().join(" "));
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
                    "invalid HTML near byte {}: {error}",
                    reader.error_position()
                ));
            }
        }
    }
    Ok(tables)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn availability_tables_map_columns_to_profiles() {
        let html = r#"<table>
            <tr><th>Command</th><th>Opcode</th><th>BF</th><th>PO</th></tr>
            <tr><td>ACI_GAP_SET_DISCOVERABLE</td><td>0xFC83</td><td>Y</td><td></td></tr>
            <tr><td>HCI_RESET</td><td>0x0C03</td><td>Y</td><td>Y</td></tr>
        </table>
        <table>
            <tr><th>Event name</th><th>LE subevent code</th><th>BF</th></tr>
            <tr><td>ACI_WARNING_EVENT</td><td>0x0006</td><td>Y</td></tr>
        </table>"#;
        let availability = parse_interface_availability(html).unwrap();
        let discoverable = &availability[&Key::Command(CommandScope::Vendor, 0xFC83)];
        assert_eq!(
            discoverable.profiles,
            [Profile::FullExtended, Profile::Full]
        );
        let reset = &availability[&Key::Command(CommandScope::Standard, 0x0C03)];
        assert_eq!(
            reset.profiles,
            [Profile::FullExtended, Profile::Full, Profile::Light]
        );
        assert!(availability.contains_key(&Key::Event(EventScope::Vendor, 0x0006)));

        let unknown = html.replace("<td>Y</td><td></td>", "<td>?</td><td></td>");
        assert!(parse_interface_availability(&unknown).is_err());
    }

    #[test]
    fn release_notes_map_binaries_to_profiles() {
        let html = r#"<table>
            <tr><th>Wireless Coprocessor Binary</th><th>stack features naming (3)</th><th>#define used in FW M0 code</th></tr>
            <tr><td>stm32wb5x_BLE_Stack_full_extended_fw.bin</td><td>-</td><td>x</td></tr>
            <tr><td>stm32wb5x_BLE_Stack_light_fw.bin</td><td>PO (Peripheral Only)</td><td>x</td></tr>
        </table>"#;
        let mapping = parse_release_notes(html).unwrap();
        assert_eq!(mapping["stm32wb5x_BLE_Stack_light_fw.bin"], Profile::Light);
        assert_eq!(
            mapping["stm32wb5x_BLE_Stack_full_extended_fw.bin"],
            Profile::FullExtended
        );

        let unknown = html.replace(
            "</table>",
            "<tr><td>stm32wb5x_BLE_Thread_static_fw.bin</td><td>Thread FTD</td><td>x</td></tr></table>",
        );
        let error = parse_release_notes(&unknown).unwrap_err();
        assert!(error.contains("unknown stack profile"), "{error}");
        let short = html.replace(
            "<td>PO (Peripheral Only)</td><td>x</td>",
            "<td>PO (Peripheral Only)</td>",
        );
        assert!(parse_release_notes(&short).is_err(), "a row missing a cell");
        assert!(same_name(
            "aci_gap_set_discoverable",
            "ACI_GAP_SET_DISCOVERABLE"
        ));
    }
}
