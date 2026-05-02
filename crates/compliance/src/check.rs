use crate::spec::{CommandSpec, FirmwareSpec};
use anyhow::{Context, Result};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

const MARKER_PREFIX: &str = "compliance:";

#[derive(Debug, Serialize)]
pub struct CoverageReport {
    pub firmware: String,
    pub rust_crate: String,
    pub commands_total: usize,
    pub rust_opcode_constants_total: usize,
    pub markers_total: usize,
    pub alias_markers_total: usize,
    pub covered_by_marker: usize,
    pub missing_markers: Vec<MissingMarker>,
    pub duplicate_markers: Vec<DuplicateMarker>,
    pub unknown_markers: Vec<UnknownMarker>,
    pub unknown_alias_markers: Vec<UnknownAliasMarker>,
    pub marker_opcode_constants_missing: Vec<MarkerOpcodeConstantMissing>,
    pub marker_opcode_mismatches: Vec<MarkerOpcodeMismatch>,
    pub marker_method_missing: Vec<MarkerMethodMissing>,
    pub rust_methods_without_marker: Vec<RustMethodWithoutMarker>,
}

#[derive(Debug, Serialize)]
pub struct MissingMarker {
    pub st_command: String,
    pub opcode: Option<u16>,
    pub expected_rust_places: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct DuplicateMarker {
    pub st_command: String,
    pub locations: Vec<MarkerLocation>,
}

#[derive(Debug, Serialize)]
pub struct UnknownMarker {
    pub st_command: String,
    pub opcode_const: String,
    pub method: Option<String>,
    pub location: MarkerLocation,
}

#[derive(Debug, Serialize)]
pub struct UnknownAliasMarker {
    pub alias_of: String,
    pub method: Option<String>,
    pub location: MarkerLocation,
}

#[derive(Debug, Serialize)]
pub struct MarkerOpcodeConstantMissing {
    pub st_command: String,
    pub opcode_const: String,
    pub method: Option<String>,
    pub location: MarkerLocation,
}

#[derive(Debug, Serialize)]
pub struct MarkerOpcodeMismatch {
    pub st_command: String,
    pub opcode_const: String,
    pub st_opcode: u16,
    pub rust_opcode: u16,
    pub method: Option<String>,
    pub location: MarkerLocation,
}

#[derive(Debug, Serialize)]
pub struct MarkerMethodMissing {
    pub st_command: String,
    pub opcode_const: String,
    pub location: MarkerLocation,
}

#[derive(Debug, Serialize)]
pub struct RustMethodWithoutMarker {
    pub method: String,
    pub location: MarkerLocation,
}

#[derive(Clone, Debug, Serialize)]
pub struct MarkerLocation {
    pub file: String,
    pub line: usize,
}

#[derive(Debug)]
struct RustOpcode {
    name: String,
    opcode: u16,
}

#[derive(Clone, Debug)]
struct CommandMarker {
    st_command: String,
    opcode_const: String,
    method: Option<String>,
    location: MarkerLocation,
}

#[derive(Clone, Debug)]
struct AliasMarker {
    alias_of: String,
    method: Option<String>,
    location: MarkerLocation,
}

#[derive(Debug)]
struct RustCommandMethod {
    name: String,
    location: MarkerLocation,
}

pub fn check_coverage(spec: &FirmwareSpec, rust_crate: &Path) -> Result<CoverageReport> {
    let opcode_path = rust_crate.join("src/vendor/opcode.rs");
    let opcodes = parse_rust_opcodes(&opcode_path)?;
    let opcode_by_name = opcodes
        .iter()
        .map(|opcode| (opcode.name.as_str(), opcode.opcode))
        .collect::<HashMap<_, _>>();
    let loaded_markers = load_command_markers(rust_crate)?;
    let markers = loaded_markers.primary;
    let alias_markers = loaded_markers.aliases;
    let rust_methods = load_rust_command_methods(rust_crate)?;
    let marked_methods = markers
        .iter()
        .filter_map(|marker| {
            marker
                .method
                .as_ref()
                .map(|method| (marker.location.file.as_str(), method.as_str()))
        })
        .chain(alias_markers.iter().filter_map(|marker| {
            marker
                .method
                .as_ref()
                .map(|method| (marker.location.file.as_str(), method.as_str()))
        }))
        .collect::<HashSet<_>>();
    let markers_by_st = group_markers_by_st(&markers);
    let st_by_formal_name = spec
        .commands
        .iter()
        .map(|command| (formal_st_name(command), command))
        .collect::<HashMap<_, _>>();

    let mut covered_by_marker = 0;
    let mut missing_markers = Vec::new();
    let mut duplicate_markers = Vec::new();
    let mut unknown_markers = Vec::new();
    let mut unknown_alias_markers = Vec::new();
    let mut marker_opcode_constants_missing = Vec::new();
    let mut marker_opcode_mismatches = Vec::new();
    let mut marker_method_missing = Vec::new();
    let rust_methods_without_marker = rust_methods
        .iter()
        .filter(|method| {
            !marked_methods.contains(&(method.location.file.as_str(), method.name.as_str()))
        })
        .map(|method| RustMethodWithoutMarker {
            method: method.name.clone(),
            location: method.location.clone(),
        })
        .collect::<Vec<_>>();

    for command in &spec.commands {
        let st_command = formal_st_name(command);
        let Some(command_markers) = markers_by_st.get(st_command.as_str()) else {
            missing_markers.push(MissingMarker {
                st_command,
                opcode: command.opcode,
                expected_rust_places: expected_places(rust_crate, command),
            });
            continue;
        };

        covered_by_marker += 1;
        if command_markers.len() > 1 {
            duplicate_markers.push(DuplicateMarker {
                st_command: st_command.clone(),
                locations: command_markers
                    .iter()
                    .map(|marker| marker.location.clone())
                    .collect(),
            });
        }

        for marker in command_markers {
            if marker.method.is_none() {
                marker_method_missing.push(MarkerMethodMissing {
                    st_command: marker.st_command.clone(),
                    opcode_const: marker.opcode_const.clone(),
                    location: marker.location.clone(),
                });
            }

            let Some(rust_opcode) = opcode_by_name.get(marker.opcode_const.as_str()) else {
                marker_opcode_constants_missing.push(MarkerOpcodeConstantMissing {
                    st_command: marker.st_command.clone(),
                    opcode_const: marker.opcode_const.clone(),
                    method: marker.method.clone(),
                    location: marker.location.clone(),
                });
                continue;
            };

            if let Some(st_opcode) = command.opcode
                && *rust_opcode != st_opcode
            {
                marker_opcode_mismatches.push(MarkerOpcodeMismatch {
                    st_command: marker.st_command.clone(),
                    opcode_const: marker.opcode_const.clone(),
                    st_opcode,
                    rust_opcode: *rust_opcode,
                    method: marker.method.clone(),
                    location: marker.location.clone(),
                });
            }
        }
    }

    for marker in &markers {
        if !st_by_formal_name.contains_key(marker.st_command.as_str()) {
            unknown_markers.push(UnknownMarker {
                st_command: marker.st_command.clone(),
                opcode_const: marker.opcode_const.clone(),
                method: marker.method.clone(),
                location: marker.location.clone(),
            });
        }
    }

    for marker in &alias_markers {
        if !st_by_formal_name.contains_key(marker.alias_of.as_str()) {
            unknown_alias_markers.push(UnknownAliasMarker {
                alias_of: marker.alias_of.clone(),
                method: marker.method.clone(),
                location: marker.location.clone(),
            });
        }
    }

    Ok(CoverageReport {
        firmware: spec.firmware.clone(),
        rust_crate: rust_crate.display().to_string(),
        commands_total: spec.commands.len(),
        rust_opcode_constants_total: opcodes.len(),
        markers_total: markers.len(),
        alias_markers_total: alias_markers.len(),
        covered_by_marker,
        missing_markers,
        duplicate_markers,
        unknown_markers,
        unknown_alias_markers,
        marker_opcode_constants_missing,
        marker_opcode_mismatches,
        marker_method_missing,
        rust_methods_without_marker,
    })
}

fn parse_rust_opcodes(path: &Path) -> Result<Vec<RustOpcode>> {
    let source = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let mut current_cgid = None;
    let mut opcodes = Vec::new();

    for line in source.lines() {
        let line = line.split("//").next().unwrap_or_default().trim();
        if line.is_empty() {
            continue;
        }

        if let Some((_, cgid)) = parse_group_cgid(line) {
            current_cgid = Some(cgid);
            continue;
        }

        let Some(cgid) = current_cgid else {
            continue;
        };
        if let Some((name, cid)) = parse_opcode_const(line) {
            let ocf = ((cgid & 0b111) << 7) | (cid & 0b111_1111);
            let opcode = (0x3f << 10) | ocf;
            opcodes.push(RustOpcode { name, opcode });
        }
    }

    Ok(opcodes)
}

fn parse_group_cgid(line: &str) -> Option<(String, u16)> {
    if line.starts_with("pub const") {
        return None;
    }
    let (name, value) = line.strip_suffix(';')?.split_once('=')?;
    Some((name.trim().to_owned(), parse_int(value.trim())?))
}

fn parse_opcode_const(line: &str) -> Option<(String, u16)> {
    let rest = line.strip_prefix("pub const ")?;
    let (name, value) = rest.strip_suffix(';')?.split_once('=')?;
    Some((name.trim().to_owned(), parse_int(value.trim())?))
}

fn parse_int(value: &str) -> Option<u16> {
    let value = value.trim();
    if let Some(hex) = value.strip_prefix("0x") {
        u16::from_str_radix(hex, 16).ok()
    } else {
        value.parse().ok()
    }
}

#[derive(Default)]
struct LoadedCommandMarkers {
    primary: Vec<CommandMarker>,
    aliases: Vec<AliasMarker>,
}

fn load_command_markers(rust_crate: &Path) -> Result<LoadedCommandMarkers> {
    let command_dir = rust_crate.join("src/vendor/command");
    let mut markers = LoadedCommandMarkers::default();

    for group in ["gap", "gatt", "hal", "l2cap"] {
        let path = command_dir.join(format!("{group}.rs"));
        let source = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let file_markers = parse_markers_in_file(&path, &source);
        markers.primary.extend(file_markers.primary);
        markers.aliases.extend(file_markers.aliases);
    }

    Ok(markers)
}

fn load_rust_command_methods(rust_crate: &Path) -> Result<Vec<RustCommandMethod>> {
    let command_dir = rust_crate.join("src/vendor/command");
    let mut methods = Vec::new();

    for group in ["gap", "gatt", "hal", "l2cap"] {
        let path = command_dir.join(format!("{group}.rs"));
        let source = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        methods.extend(parse_trait_methods_in_file(&path, &source));
    }

    Ok(methods)
}

fn parse_markers_in_file(path: &Path, source: &str) -> LoadedCommandMarkers {
    let lines = source.lines().collect::<Vec<_>>();
    let mut markers = LoadedCommandMarkers::default();

    for (idx, line) in lines.iter().enumerate() {
        let Some(marker) = parse_marker_line(line) else {
            continue;
        };

        let location = MarkerLocation {
            file: path.display().to_string(),
            line: idx + 1,
        };
        match marker {
            MarkerKind::Primary { st, opcode } => {
                markers.primary.push(CommandMarker {
                    st_command: st,
                    opcode_const: opcode,
                    method: next_method_name(&lines, idx + 1),
                    location,
                });
            }
            MarkerKind::Alias { alias_of } => {
                markers.aliases.push(AliasMarker {
                    alias_of,
                    method: next_method_name(&lines, idx + 1),
                    location,
                });
            }
        }
    }

    markers
}

fn parse_trait_methods_in_file(path: &Path, source: &str) -> Vec<RustCommandMethod> {
    let mut methods = Vec::new();
    let mut in_command_trait = false;
    let mut trait_depth = 0isize;

    for (idx, line) in source.lines().enumerate() {
        let code = line.split("//").next().unwrap_or_default();
        let trimmed = code.trim();

        if !in_command_trait
            && trimmed.starts_with("pub trait ")
            && trimmed.contains("Commands")
            && trimmed.contains('{')
        {
            in_command_trait = true;
        }

        if in_command_trait && let Some(method) = parse_fn_name(trimmed) {
            methods.push(RustCommandMethod {
                name: method,
                location: MarkerLocation {
                    file: path.display().to_string(),
                    line: idx + 1,
                },
            });
        }

        if in_command_trait {
            trait_depth += count_char(code, '{') as isize;
            trait_depth -= count_char(code, '}') as isize;
            if trait_depth <= 0 {
                in_command_trait = false;
                trait_depth = 0;
            }
        }
    }

    methods
}

enum MarkerKind {
    Primary { st: String, opcode: String },
    Alias { alias_of: String },
}

fn parse_marker_line(line: &str) -> Option<MarkerKind> {
    let line = line.trim().strip_prefix("//")?.trim();
    let marker = line.strip_prefix(MARKER_PREFIX)?.trim();
    let mut st = None;
    let mut opcode = None;
    let mut alias_of = None;

    for part in marker.split_whitespace() {
        if let Some(value) = part.strip_prefix("st=") {
            st = Some(value.to_owned());
        } else if let Some(value) = part.strip_prefix("opcode=") {
            opcode = Some(value.to_owned());
        } else if let Some(value) = part.strip_prefix("alias_of=") {
            alias_of = Some(value.to_owned());
        }
    }

    if let Some(alias_of) = alias_of {
        Some(MarkerKind::Alias { alias_of })
    } else {
        Some(MarkerKind::Primary {
            st: st?,
            opcode: opcode?,
        })
    }
}

fn count_char(source: &str, needle: char) -> usize {
    source.chars().filter(|c| *c == needle).count()
}

fn next_method_name(lines: &[&str], start: usize) -> Option<String> {
    for line in lines.iter().skip(start) {
        let trimmed = line.trim();
        if trimmed.starts_with("//") && trimmed.contains(MARKER_PREFIX) {
            return None;
        }
        if let Some(method) = parse_fn_name(trimmed) {
            return Some(method);
        }
    }

    None
}

fn parse_fn_name(line: &str) -> Option<String> {
    let fn_pos = line.find("fn ")?;
    let rest = &line[fn_pos + 3..];
    let name = rest
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect::<String>();
    (!name.is_empty()).then_some(name)
}

fn group_markers_by_st(markers: &[CommandMarker]) -> HashMap<String, Vec<&CommandMarker>> {
    let mut out = HashMap::<String, Vec<&CommandMarker>>::new();
    for marker in markers {
        out.entry(marker.st_command.clone())
            .or_default()
            .push(marker);
    }
    out
}

fn formal_st_name(command: &CommandSpec) -> String {
    command
        .doc
        .as_ref()
        .and_then(|doc| doc.brief.as_deref())
        .filter(|brief| brief.starts_with("ACI_"))
        .map(str::to_owned)
        .unwrap_or_else(|| command.name.to_ascii_uppercase())
}

fn expected_places(rust_crate: &Path, command: &CommandSpec) -> Vec<String> {
    let mut paths = vec![
        rust_crate
            .join("src/vendor/opcode.rs")
            .display()
            .to_string(),
    ];
    if let Some(command_file) = command_file(rust_crate, command.group.as_str()) {
        paths.push(command_file.display().to_string());
    }
    paths
}

fn command_file(rust_crate: &Path, group: &str) -> Option<PathBuf> {
    match group {
        "gap" | "gatt" | "hal" | "l2cap" => {
            Some(rust_crate.join(format!("src/vendor/command/{group}.rs")))
        }
        _ => None,
    }
}
