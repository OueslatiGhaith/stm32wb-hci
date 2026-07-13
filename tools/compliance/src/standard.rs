//! Discovery of the public standard-HCI provider used by this crate.
//!
//! STM32CubeWB's `ble_hci_le.c` describes standard HCI commands, while this
//! crate delegates most of that surface to the direct `bt-hci` dependency. The
//! dependency must be public (`pub use bt_hci`) before it can count as crate
//! coverage. A small number of STM32WB commands not yet present upstream live
//! in `src/standard.rs` and are discovered alongside it.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use syn::{Attribute, Item};

use crate::FirmwareVersion;
use crate::model::{CoverageEntry, CoverageOrigin};

/// The standard HCI API surface that callers can access through this crate.
#[derive(Clone, Debug, Default)]
pub(crate) struct StandardProviderCoverage {
    /// Full HCI opcodes, rather than vendor OCFs.
    pub(crate) commands: Vec<CoverageEntry>,
    /// Ordinary HCI event codes.
    pub(crate) events: Vec<CoverageEntry>,
    /// LE Meta Event subevent codes.
    pub(crate) le_meta_events: Vec<CoverageEntry>,
}

/// Load the public standard-HCI provider and STM32WB's local standard-command
/// extensions for the selected firmware.
pub(crate) fn load_standard_provider_coverage(
    crate_dir: &Path,
    firmware: FirmwareVersion,
) -> Result<StandardProviderCoverage, String> {
    require_public_bt_hci_reexport(crate_dir)?;

    let bt_hci_dir = find_bt_hci_source(crate_dir)?;
    let mut coverage = StandardProviderCoverage::default();
    coverage.commands.extend(load_bt_hci_commands(&bt_hci_dir)?);
    coverage.events.extend(load_bt_hci_events(
        &bt_hci_dir.join("src/event.rs"),
        CoverageOrigin::StandardHciProvider,
    )?);
    coverage.le_meta_events.extend(load_bt_hci_events(
        &bt_hci_dir.join("src/event/le.rs"),
        CoverageOrigin::StandardHciProvider,
    )?);

    // `bt-hci` does not currently define a handful of STM32WB-supported LE
    // commands. These are public raw command descriptors in the crate itself.
    coverage.commands.extend(load_local_command_macros(
        &crate_dir.join("src/standard.rs"),
        firmware,
        CoverageOrigin::StandardHciExtension,
    )?);

    sort_and_deduplicate(&mut coverage.commands);
    sort_and_deduplicate(&mut coverage.events);
    sort_and_deduplicate(&mut coverage.le_meta_events);
    Ok(coverage)
}

fn require_public_bt_hci_reexport(crate_dir: &Path) -> Result<(), String> {
    let path = crate_dir.join("src/lib.rs");
    let source = fs::read_to_string(&path)
        .map_err(|error| format!("could not read {}: {error}", path.display()))?;
    let file = syn::parse_file(&source)
        .map_err(|error| format!("could not parse {}: {error}", path.display()))?;

    let is_reexported = file.items.iter().any(|item| {
        let Item::Use(item) = item else {
            return false;
        };
        matches!(item.vis, syn::Visibility::Public(_)) && use_tree_mentions_bt_hci(&item.tree)
    });
    if is_reexported {
        Ok(())
    } else {
        Err(format!(
            "{} depends on bt-hci but does not publicly re-export it; standard HCI coverage cannot be claimed",
            path.display()
        ))
    }
}

fn use_tree_mentions_bt_hci(tree: &syn::UseTree) -> bool {
    match tree {
        syn::UseTree::Path(path) => path.ident == "bt_hci" || use_tree_mentions_bt_hci(&path.tree),
        syn::UseTree::Name(name) => name.ident == "bt_hci",
        syn::UseTree::Rename(rename) => rename.ident == "bt_hci",
        syn::UseTree::Group(group) => group.items.iter().any(use_tree_mentions_bt_hci),
        syn::UseTree::Glob(_) => false,
    }
}

fn find_bt_hci_source(crate_dir: &Path) -> Result<PathBuf, String> {
    if let Some(path) = env::var_os("STM32WB_COMPLIANCE_BT_HCI_SOURCE") {
        let path = PathBuf::from(path);
        if path.join("src/cmd.rs").is_file() && path.join("src/event.rs").is_file() {
            return Ok(path);
        }
        return Err(format!(
            "STM32WB_COMPLIANCE_BT_HCI_SOURCE={} is not a bt-hci source directory",
            path.display()
        ));
    }

    let version = locked_bt_hci_version(&crate_dir.join("Cargo.lock"))?;
    let cargo_home = env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".cargo")))
        .ok_or_else(|| {
            "could not locate CARGO_HOME; set STM32WB_COMPLIANCE_BT_HCI_SOURCE explicitly"
                .to_owned()
        })?;
    let registry_sources = cargo_home.join("registry/src");
    let entries = fs::read_dir(&registry_sources).map_err(|error| {
        format!(
            "could not read {}; run cargo check first or set STM32WB_COMPLIANCE_BT_HCI_SOURCE: {error}",
            registry_sources.display()
        )
    })?;

    let directory_name = format!("bt-hci-{version}");
    let mut candidates = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path().join(&directory_name))
        .filter(|path| path.join("src/cmd.rs").is_file() && path.join("src/event.rs").is_file())
        .collect::<Vec<_>>();
    candidates.sort();
    candidates.into_iter().next().ok_or_else(|| {
        format!(
            "could not find bt-hci {version} in {}; run cargo check first or set STM32WB_COMPLIANCE_BT_HCI_SOURCE",
            registry_sources.display()
        )
    })
}

fn locked_bt_hci_version(lockfile: &Path) -> Result<String, String> {
    let lock = fs::read_to_string(lockfile)
        .map_err(|error| format!("could not read {}: {error}", lockfile.display()))?;
    for package in lock.split("[[package]]").skip(1) {
        let mut name = None;
        let mut version = None;
        for line in package.lines() {
            let line = line.trim();
            if let Some(value) = quoted_toml_value(line, "name") {
                name = Some(value);
            } else if let Some(value) = quoted_toml_value(line, "version") {
                version = Some(value);
            }
        }
        if name.as_deref() == Some("bt-hci") {
            return version.ok_or_else(|| "bt-hci package in Cargo.lock has no version".to_owned());
        }
    }
    Err(format!(
        "{} does not contain a bt-hci package",
        lockfile.display()
    ))
}

fn quoted_toml_value(line: &str, key: &str) -> Option<String> {
    let value = line
        .strip_prefix(key)?
        .trim_start()
        .strip_prefix('=')?
        .trim();
    Some(value.strip_prefix('"')?.strip_suffix('"')?.to_owned())
}

fn load_bt_hci_commands(bt_hci_dir: &Path) -> Result<Vec<CoverageEntry>, String> {
    let mut commands = Vec::new();
    for path in public_bt_hci_command_modules(bt_hci_dir)? {
        commands.extend(load_command_macros_from_file(
            &path,
            FirmwareVersion::new(u16::MAX, u16::MAX, u16::MAX),
            CoverageOrigin::StandardHciProvider,
            false,
        )?);
    }
    Ok(commands)
}

/// Follow the public module declarations in `bt-hci/src/cmd.rs` rather than
/// recursively scanning every source file in the dependency. This makes a
/// private helper command impossible to satisfy a public CubeWB API claim.
fn public_bt_hci_command_modules(bt_hci_dir: &Path) -> Result<Vec<PathBuf>, String> {
    let root = bt_hci_dir.join("src/cmd.rs");
    let source = fs::read_to_string(&root)
        .map_err(|error| format!("could not read {}: {error}", root.display()))?;
    let file = syn::parse_file(&source)
        .map_err(|error| format!("could not parse {}: {error}", root.display()))?;
    let command_dir = bt_hci_dir.join("src/cmd");
    let mut paths = Vec::new();
    for item in file.items {
        let Item::Mod(module) = item else {
            continue;
        };
        if !matches!(module.vis, syn::Visibility::Public(_)) || module.content.is_some() {
            continue;
        }
        let path = command_dir.join(format!("{}.rs", module.ident));
        if !path.is_file() {
            return Err(format!(
                "{} declares public command module `{}` but {} does not exist",
                root.display(),
                module.ident,
                path.display()
            ));
        }
        paths.push(path);
    }
    if paths.is_empty() {
        return Err(format!(
            "{} declares no public bt-hci command modules",
            root.display()
        ));
    }
    paths.sort();
    Ok(paths)
}

fn load_local_command_macros(
    path: &Path,
    firmware: FirmwareVersion,
    origin: CoverageOrigin,
) -> Result<Vec<CoverageEntry>, String> {
    load_command_macros_from_file(path, firmware, origin, true)
}

fn load_command_macros_from_file(
    path: &Path,
    firmware: FirmwareVersion,
    origin: CoverageOrigin,
    honor_cfg: bool,
) -> Result<Vec<CoverageEntry>, String> {
    let source = fs::read_to_string(path)
        .map_err(|error| format!("could not read {}: {error}", path.display()))?;
    let file = syn::parse_file(&source)
        .map_err(|error| format!("could not parse {}: {error}", path.display()))?;
    let mut entries = Vec::new();
    for item in &file.items {
        let Item::Macro(item) = item else {
            continue;
        };
        if !macro_name_is(&item.mac, "cmd") || (honor_cfg && !attrs_active(&item.attrs, firmware)?)
        {
            continue;
        }
        let Some(header) = parse_command_macro_header(&item.mac.tokens.to_string()) else {
            continue;
        };
        let Some(ogf) = standard_ogf(&header.group) else {
            // `BASE` macro implementation details and non-standard groups are
            // deliberately not treated as public standard command descriptors.
            continue;
        };
        if header.ocf > 0x03ff {
            return Err(format!(
                "{}: standard command {} has OCF 0x{:X}, which exceeds ten bits",
                path.display(),
                header.name,
                header.ocf
            ));
        }
        entries.push(
            CoverageEntry::new((u16::from(ogf) << 10) | header.ocf, header.name, origin)
                .at(path.to_path_buf()),
        );
    }
    Ok(entries)
}

fn load_bt_hci_events(path: &Path, origin: CoverageOrigin) -> Result<Vec<CoverageEntry>, String> {
    let source = fs::read_to_string(path)
        .map_err(|error| format!("could not read {}: {error}", path.display()))?;
    let file = syn::parse_file(&source)
        .map_err(|error| format!("could not parse {}: {error}", path.display()))?;
    let mut events = Vec::new();
    for item in &file.items {
        let Item::Macro(item) = item else {
            continue;
        };
        if !macro_name_is(&item.mac, "events") && !macro_name_is(&item.mac, "le_events") {
            continue;
        }
        for (name, code) in parse_event_macro_headers(&item.mac.tokens.to_string()) {
            if code > u16::from(u8::MAX) {
                return Err(format!(
                    "{}: HCI event {name} has out-of-range code 0x{code:X}",
                    path.display()
                ));
            }
            events.push(CoverageEntry::new(code, name, origin).at(path.to_path_buf()));
        }
    }
    if events.is_empty() {
        return Err(format!(
            "{}: no standard event declarations were found",
            path.display()
        ));
    }
    Ok(events)
}

fn macro_name_is(mac: &syn::Macro, name: &str) -> bool {
    mac.path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == name)
}

struct CommandMacroHeader {
    name: String,
    group: String,
    ocf: u16,
}

/// Parse the stable header shared by `bt-hci::cmd!` invocations:
/// `Name(GROUP, 0x0123) { ... }`.
fn parse_command_macro_header(tokens: &str) -> Option<CommandMacroHeader> {
    let tokens = strip_leading_attributes(tokens.trim())?;
    // `bt-hci::cmd!` expands its convenient public forms through a second
    // `cmd! { BASE ... }` invocation. We inventory source macro invocations,
    // so recognize both the direct and BASE forms.
    let tokens = strip_base_marker(tokens);
    let tokens = strip_leading_attributes(tokens)?;
    let open = tokens.find('(')?;
    let name = tokens[..open].trim();
    if !is_identifier(name) {
        return None;
    }
    let close = matching_parenthesis(tokens, open)?;
    let arguments = &tokens[open + 1..close];
    let (group, ocf) = arguments.split_once(',')?;
    let group = group.trim();
    let ocf = parse_integer(ocf.trim())?;
    Some(CommandMacroHeader {
        name: name.to_owned(),
        group: group.to_owned(),
        ocf,
    })
}

fn strip_base_marker(tokens: &str) -> &str {
    let Some(rest) = tokens.strip_prefix("BASE") else {
        return tokens;
    };
    if rest
        .as_bytes()
        .first()
        .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
    {
        tokens
    } else {
        rest.trim_start()
    }
}

/// `syn::Macro::tokens` retains doc comments as `#[doc = ...]` attributes.
/// They precede the command header in the public `bt-hci` sources, so remove
/// them structurally rather than making command discovery depend on formatting.
fn strip_leading_attributes(mut tokens: &str) -> Option<&str> {
    loop {
        tokens = tokens.trim_start();
        if !tokens.starts_with('#') {
            return Some(tokens);
        }
        let open = tokens.find('[')?;
        let close = matching_delimiter(tokens, open, b'[', b']')?;
        tokens = &tokens[close + 1..];
    }
}

/// Parse `struct Name(0xNN) { ... }` declarations inside `events!` macros.
fn parse_event_macro_headers(tokens: &str) -> Vec<(String, u16)> {
    let mut entries = Vec::new();
    let mut cursor = 0;
    while let Some(relative) = tokens[cursor..].find("struct ") {
        let start = cursor + relative + "struct ".len();
        let rest = &tokens[start..];
        let name_end = rest
            .bytes()
            .take_while(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
            .count();
        let name = &rest[..name_end];
        if !is_identifier(name) {
            cursor = start;
            continue;
        }
        let mut after_name = start + name_end;
        while tokens
            .as_bytes()
            .get(after_name)
            .is_some_and(u8::is_ascii_whitespace)
        {
            after_name += 1;
        }
        // Lifetime parameters are irrelevant to the wire code.
        if tokens.as_bytes().get(after_name) == Some(&b'<') {
            let Some(close) = matching_angle(tokens, after_name) else {
                cursor = after_name + 1;
                continue;
            };
            after_name = close + 1;
        }
        while tokens
            .as_bytes()
            .get(after_name)
            .is_some_and(u8::is_ascii_whitespace)
        {
            after_name += 1;
        }
        if tokens.as_bytes().get(after_name) != Some(&b'(') {
            cursor = after_name;
            continue;
        }
        let Some(close) = matching_parenthesis(tokens, after_name) else {
            cursor = after_name + 1;
            continue;
        };
        if let Some(code) = parse_integer(tokens[after_name + 1..close].trim()) {
            entries.push((name.to_owned(), code));
        }
        cursor = close + 1;
    }
    entries
}

fn attrs_active(attributes: &[Attribute], firmware: FirmwareVersion) -> Result<bool, String> {
    attributes
        .iter()
        .filter(|attribute| attribute.path().is_ident("cfg"))
        .try_fold(true, |active, attribute| {
            let meta = attribute
                .meta
                .require_list()
                .map_err(|error| format!("malformed cfg attribute: {error}"))?;
            Ok(active && eval_cfg_meta(&meta.tokens.to_string(), firmware)?)
        })
}

fn eval_cfg_meta(expression: &str, firmware: FirmwareVersion) -> Result<bool, String> {
    let expression = expression.trim();
    if let Some(inner) = wrapped_expression(expression, "all") {
        return split_top_level(inner, ',')
            .into_iter()
            .try_fold(true, |active, part| {
                Ok(active && eval_cfg_meta(part, firmware)?)
            });
    }
    if let Some(inner) = wrapped_expression(expression, "any") {
        let mut result = false;
        for part in split_top_level(inner, ',') {
            result |= eval_cfg_meta(part, firmware)?;
        }
        return Ok(result);
    }
    if let Some(inner) = wrapped_expression(expression, "not") {
        return Ok(!eval_cfg_meta(inner, firmware)?);
    }
    if let Some((key, value)) = expression.split_once('=') {
        return Ok(
            key.trim() == "feature" && value.trim().trim_matches('"') == firmware.feature_name()
        );
    }
    firmware.matches_version_cfg(expression).ok_or_else(|| {
        format!("unsupported cfg predicate {expression:?} while reading standard command coverage")
    })
}

fn wrapped_expression<'a>(expression: &'a str, name: &str) -> Option<&'a str> {
    let rest = expression
        .strip_prefix(name)?
        .trim_start()
        .strip_prefix('(')?;
    rest.strip_suffix(')')
}

fn split_top_level(input: &str, delimiter: char) -> Vec<&str> {
    let mut values = Vec::new();
    let mut start = 0;
    let mut depth = 0usize;
    for (index, character) in input.char_indices() {
        match character {
            '(' | '[' | '{' | '<' => depth += 1,
            ')' | ']' | '}' | '>' => depth = depth.saturating_sub(1),
            character if character == delimiter && depth == 0 => {
                values.push(input[start..index].trim());
                start = index + character.len_utf8();
            }
            _ => {}
        }
    }
    values.push(input[start..].trim());
    values
}

fn standard_ogf(group: &str) -> Option<u8> {
    match group {
        "LINK_CONTROL" => Some(0x01),
        "LINK_POLICY" => Some(0x02),
        "CONTROL_BASEBAND" => Some(0x03),
        "INFO_PARAMS" => Some(0x04),
        "STATUS_PARAMS" => Some(0x05),
        "TESTING" => Some(0x06),
        "LE" => Some(0x08),
        _ => None,
    }
}

fn parse_integer(value: &str) -> Option<u16> {
    let value = value.trim().trim_end_matches(['u', 'U', 'l', 'L']);
    if let Some(value) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        u16::from_str_radix(value, 16).ok()
    } else {
        value.parse().ok()
    }
}

fn matching_parenthesis(source: &str, open: usize) -> Option<usize> {
    matching_delimiter(source, open, b'(', b')')
}

fn matching_angle(source: &str, open: usize) -> Option<usize> {
    // Lifetime parameters (`<'a>`) contain apostrophes, which are not quoted
    // strings. The generic delimiter parser deliberately skips quoted literals,
    // so use this small angle-only matcher here instead.
    let bytes = source.as_bytes();
    let mut depth = 0usize;
    for (index, byte) in bytes.iter().enumerate().skip(open) {
        match byte {
            b'<' => depth += 1,
            b'>' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

fn matching_delimiter(source: &str, open: usize, opening: u8, closing: u8) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut depth = 0usize;
    let mut index = open;
    while index < bytes.len() {
        match bytes[index] {
            byte if byte == opening => depth += 1,
            byte if byte == closing => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(index);
                }
            }
            b'\'' | b'"' => index = skip_quoted(bytes, index)?,
            _ => {}
        }
        index += 1;
    }
    None
}

fn skip_quoted(bytes: &[u8], quote: usize) -> Option<usize> {
    let delimiter = bytes[quote];
    let mut index = quote + 1;
    while index < bytes.len() {
        if bytes[index] == b'\\' {
            index += 2;
            continue;
        }
        if bytes[index] == delimiter {
            return Some(index);
        }
        index += 1;
    }
    None
}

fn is_identifier(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn sort_and_deduplicate(entries: &mut Vec<CoverageEntry>) {
    entries.sort_by_key(|entry| (entry.code, entry.name.clone()));
    entries.dedup_by(|left, right| left.code == right.code && left.name == right.name);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_standard_command_headers() {
        let header = parse_command_macro_header(
            "LeSetAdvData ( LE , 0x0008 ) { Params = [u8 ; 32] ; Return = () ; }",
        )
        .unwrap();
        assert_eq!(header.name, "LeSetAdvData");
        assert_eq!(header.group, "LE");
        assert_eq!(header.ocf, 8);
    }

    #[test]
    fn ignores_doc_attributes_before_a_command_header() {
        let header = parse_command_macro_header(
            "# [ doc = \"command\" ] LeTest ( LE , 0x001F ) { Params = () ; }",
        )
        .unwrap();
        assert_eq!(header.name, "LeTest");
    }

    #[test]
    fn parses_bt_hci_base_command_headers() {
        let header = parse_command_macro_header(
            "BASE # [ doc = \"command\" ] LeExtended ( LE , 0x0041 ) { Params = () ; }",
        )
        .unwrap();
        assert_eq!(header.name, "LeExtended");
        assert_eq!(header.ocf, 0x41);
    }

    #[test]
    fn parses_event_macro_headers_with_lifetimes() {
        let events = parse_event_macro_headers(
            "struct ConnectionComplete ( 0x03 ) { } struct LeAdvertisingReport < 'a > ( 0x02 ) { }",
        );
        assert_eq!(
            events,
            vec![
                ("ConnectionComplete".into(), 3),
                ("LeAdvertisingReport".into(), 2),
            ]
        );
    }

    #[test]
    fn honors_firmware_cfg_on_local_extension() {
        let old = FirmwareVersion::new(0, 16, 0);
        let new = FirmwareVersion::new(0, 17, 0);
        assert!(!eval_cfg_meta("any ( only_fw_0_17_0 , after_fw_0_17_0 )", old).unwrap());
        assert!(eval_cfg_meta("any ( only_fw_0_17_0 , after_fw_0_17_0 )", new).unwrap());
    }
}
