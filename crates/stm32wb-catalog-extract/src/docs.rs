//! Layer 2: facts from ST's tagged HTML documents.
//!
//! - `STM32WB_BLE_Wireless_Interface.html` and
//!   `STM32WBA_BLE_Wireless_Interface.html` list every command and event with
//!   one availability column per reduced stack profile (BF, PO, LO, LB, BO on
//!   STM32WB; BP, BF, PO, LO, and LB before 1.8.0, on STM32WBA, which added
//!   BP and PO in 1.5.0). The platform's complete profile supports the whole
//!   interface, and a release has the profiles its document has columns for.
//!   Each entry's section lists the events it generates, which name the
//!   completion event of a command.
//! - Each family's `Release_Notes.html` maps binary files to stack profiles.

use std::collections::{BTreeMap, BTreeSet};

use quick_xml::Reader;
use quick_xml::events::Event;
use stm32wb_catalog::{
    CommandScope, Completion, EventScope, Family, Platform, Profile, SnapshotBinary,
};

use crate::cube::{BINARIES_DIR, CubeTag};

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

/// The entries the interface document at `path` lists, with the profiles of
/// `platform` supporting each.
pub fn interface_availability(
    tag: &CubeTag,
    path: &str,
    platform: Platform,
) -> Result<BTreeMap<Key, Documented>, String> {
    let source = tag.read_text(path)?;
    parse_interface_availability(&source, platform).map_err(|error| format!("{path}: {error}"))
}

/// The stack profiles the interface document at `path` has: the complete
/// one, and those its availability tables have columns for.
pub fn interface_profiles(
    tag: &CubeTag,
    path: &str,
    platform: Platform,
) -> Result<Vec<Profile>, String> {
    let source = tag.read_text(path)?;
    parse_interface_profiles(&source, platform).map_err(|error| format!("{path}: {error}"))
}

fn parse_interface_profiles(source: &str, platform: Platform) -> Result<Vec<Profile>, String> {
    let mut profiles = BTreeSet::from([platform.complete_profile()]);
    for table in parse_html_tables(source)? {
        let Some(header) = table.first() else {
            continue;
        };
        if table_kind(header).is_some() {
            for (_, profile) in profile_columns(header, platform)? {
                profiles.insert(profile);
            }
        }
    }
    Ok(profiles.into_iter().collect())
}

/// The index and profile of each availability column of a table header.
fn profile_columns(header: &[String], platform: Platform) -> Result<Vec<(usize, Profile)>, String> {
    let mut columns = Vec::new();
    for (index, heading) in header.iter().enumerate().skip(2) {
        let profile = platform
            .profile_for_column(heading)
            .ok_or_else(|| format!("unknown availability column {heading:?}"))?;
        columns.push((index, profile));
    }
    if columns.is_empty() {
        return Err(format!(
            "availability table without profile columns: {header:?}"
        ));
    }
    Ok(columns)
}

fn parse_interface_availability(
    source: &str,
    platform: Platform,
) -> Result<BTreeMap<Key, Documented>, String> {
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
        let columns = profile_columns(header, platform)?;
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
            let mut profiles = BTreeSet::from([platform.complete_profile()]);
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

/// The completion event each documented command's "Events generated" list
/// names, keyed by its documented name, or `None` where the list names
/// neither.
pub fn command_completions(
    tag: &CubeTag,
    path: &str,
) -> Result<BTreeMap<String, Option<Completion>>, String> {
    let source = tag.read_text(path)?;
    parse_command_completions(&source).map_err(|error| format!("{path}: {error}"))
}

fn parse_command_completions(source: &str) -> Result<BTreeMap<String, Option<Completion>>, String> {
    let mut completions = BTreeMap::new();
    for (name, events) in generated_events(source)? {
        let status = events
            .iter()
            .any(|event| event == "HCI_COMMAND_STATUS_EVENT");
        let complete = events
            .iter()
            .any(|event| event == "HCI_COMMAND_COMPLETE_EVENT");
        let completion = match (status, complete) {
            (true, true) => {
                return Err(format!(
                    "{name} generates both Command Status and Command Complete"
                ));
            }
            (true, false) => Some(Completion::CommandStatus),
            (false, true) => Some(Completion::CommandComplete),
            (false, false) => None,
        };
        if completions.insert(name.clone(), completion).is_some() {
            return Err(format!("{name} lists the events it generates twice"));
        }
    }
    Ok(completions)
}

/// `(section, items)` for every `<h2>` section with an "Events generated"
/// `<h3>` subsection, whose `<li>` items name the events.
fn generated_events(source: &str) -> Result<Vec<(String, Vec<String>)>, String> {
    let mut reader = Reader::from_str(source);
    reader.config_mut().check_end_names = false;
    reader.config_mut().allow_unmatched_ends = true;

    let mut sections: Vec<(String, Vec<String>)> = Vec::new();
    let mut section: Option<String> = None;
    let mut listing = false;
    // The text of the heading or item being read.
    let mut text: Option<String> = None;
    loop {
        match reader.read_event() {
            Ok(Event::Start(tag)) => match tag.local_name().as_ref() {
                b"h1" | b"h2" | b"h3" => {
                    listing = false;
                    text = Some(String::new());
                }
                b"li" if listing => text = Some(String::new()),
                _ => {}
            },
            Ok(Event::Text(content)) => {
                if let Some(text) = text.as_mut() {
                    text.push_str(
                        &content
                            .html_content()
                            .map_err(|error| format!("could not decode HTML text: {error}"))?,
                    );
                }
            }
            Ok(Event::End(tag)) => {
                let finished = || {
                    text.as_deref()
                        .map(|value| value.split_whitespace().collect::<Vec<_>>().join(" "))
                };
                match tag.local_name().as_ref() {
                    b"h1" => {
                        section = None;
                        text = None;
                    }
                    b"h2" => {
                        section = finished();
                        text = None;
                    }
                    b"h3" => {
                        if finished().as_deref() == Some("Events generated") {
                            let name = section
                                .clone()
                                .ok_or("an \"Events generated\" list belongs to no section")?;
                            sections.push((name, Vec::new()));
                            listing = true;
                        }
                        text = None;
                    }
                    b"li" if listing => {
                        if let (Some(item), Some((_, items))) = (finished(), sections.last_mut()) {
                            items.push(item);
                        }
                        text = None;
                    }
                    _ => {}
                }
            }
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
    if sections.is_empty() {
        return Err("no \"Events generated\" lists were found".to_owned());
    }
    Ok(sections)
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
pub fn binaries(tag: &CubeTag) -> Result<Vec<SnapshotBinary>, String> {
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
        for &profile in Platform::Stm32wb.profiles() {
            let Some(file) = family.binary_file_name(profile) else {
                continue;
            };
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
                    .and_then(|column| Platform::Stm32wb.profile_for_column(column))
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
        let availability = parse_interface_availability(html, Platform::Stm32wb).unwrap();
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
        assert!(parse_interface_availability(&unknown, Platform::Stm32wb).is_err());

        // Columns name the profiles of the document's platform.
        let wba = html.replace("<th>PO</th>", "<th>BP</th>");
        assert!(parse_interface_availability(&wba, Platform::Stm32wb).is_err());
        let availability = parse_interface_availability(&wba, Platform::Stm32wba).unwrap();
        assert_eq!(
            availability[&Key::Command(CommandScope::Standard, 0x0C03)].profiles,
            [
                Profile::WbaFull,
                Profile::WbaBasicPlus,
                Profile::WbaBasicFeatures
            ]
        );
    }

    #[test]
    fn completions_come_from_the_events_generated() {
        let html = r##"<h2><a name="HCI_DISCONNECT_anchor">HCI_DISCONNECT</a></h2>
            <h3>Description</h3><p>Ends a connection with <li>HCI_COMMAND_COMPLETE_EVENT</li></p>
            <h3>Events generated</h3>
            <li><a href="#x">HCI_COMMAND_STATUS_EVENT</a></li>
            <li><a href="#y">HCI_DISCONNECTION_COMPLETE_EVENT</a></li>
            <h2><a name="HCI_RESET_anchor">HCI_RESET</a></h2>
            <h3>Events generated</h3><li><a href="#z">HCI_COMMAND_COMPLETE_EVENT</a></li>
            <h2><a name="HCI_HOST_NUMBER_OF_COMPLETED_PACKETS_anchor">HCI_HOST_NUMBER_OF_COMPLETED_PACKETS</a></h2>
            <h3>Events generated</h3><p>Normally, no event is generated.</p>
            <h2><a name="HCI_NO_LIST_anchor">HCI_NO_LIST</a></h2>
            <h3>Description</h3><p>No list.</p>
            <h1>Revision history</h1><li>HCI_COMMAND_STATUS_EVENT</li>"##;
        let completions = parse_command_completions(html).unwrap();
        assert_eq!(
            completions,
            BTreeMap::from([
                ("HCI_DISCONNECT".to_owned(), Some(Completion::CommandStatus)),
                ("HCI_HOST_NUMBER_OF_COMPLETED_PACKETS".to_owned(), None),
                ("HCI_RESET".to_owned(), Some(Completion::CommandComplete)),
            ])
        );

        let both = html.replace(
            "HCI_DISCONNECTION_COMPLETE_EVENT",
            "HCI_COMMAND_COMPLETE_EVENT",
        );
        let error = parse_command_completions(&both).unwrap_err();
        assert!(error.contains("both"), "{error}");
        let twice = html.replace("HCI_RESET", "HCI_DISCONNECT");
        let error = parse_command_completions(&twice).unwrap_err();
        assert!(error.contains("twice"), "{error}");
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
