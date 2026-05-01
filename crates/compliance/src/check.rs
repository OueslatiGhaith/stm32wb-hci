use crate::spec::{CommandSpec, FirmwareSpec};
use anyhow::{Context, Result};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize)]
pub struct CoverageReport {
    pub firmware: String,
    pub rust_crate: String,
    pub commands_total: usize,
    pub rust_opcode_constants_total: usize,
    pub covered_by_opcode: usize,
    pub obvious_command_impl_matches: usize,
    pub missing_opcode_constants: Vec<MissingOpcodeConstant>,
    pub opcode_value_mismatches: Vec<OpcodeValueMismatch>,
    pub missing_command_impls: Vec<MissingCommandImpl>,
}

#[derive(Debug, Serialize)]
pub struct MissingOpcodeConstant {
    pub st_command: String,
    pub opcode: Option<u16>,
    pub expected_rust_const: String,
    pub expected_rust_places: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct OpcodeValueMismatch {
    pub st_command: String,
    pub expected_rust_const: String,
    pub st_opcode: u16,
    pub rust_opcode: u16,
}

#[derive(Debug, Serialize)]
pub struct MissingCommandImpl {
    pub st_command: String,
    pub opcode: Option<u16>,
    pub expected_rust_method_fragments: Vec<String>,
    pub expected_rust_places: Vec<String>,
}

#[derive(Debug)]
struct RustOpcode {
    name: String,
    opcode: u16,
}

pub fn check_coverage(spec: &FirmwareSpec, rust_crate: &Path) -> Result<CoverageReport> {
    let opcode_path = rust_crate.join("src/vendor/opcode.rs");
    let opcodes = parse_rust_opcodes(&opcode_path)?;
    let opcode_by_value = opcodes
        .iter()
        .map(|opcode| (opcode.opcode, opcode.name.as_str()))
        .collect::<HashMap<_, _>>();
    let opcode_by_name = opcodes
        .iter()
        .map(|opcode| (opcode.name.as_str(), opcode.opcode))
        .collect::<HashMap<_, _>>();
    let command_identifiers = load_command_identifiers(rust_crate)?;

    let mut covered_by_opcode = 0;
    let mut obvious_command_impl_matches = 0;
    let mut missing_opcode_constants = Vec::new();
    let mut opcode_value_mismatches = Vec::new();
    let mut missing_command_impls = Vec::new();

    for command in &spec.commands {
        let expected_const = expected_rust_const(command);
        if let Some(st_opcode) = command.opcode {
            if opcode_by_value.contains_key(&st_opcode) {
                covered_by_opcode += 1;
            } else {
                missing_opcode_constants.push(MissingOpcodeConstant {
                    st_command: command.name.clone(),
                    opcode: command.opcode,
                    expected_rust_const: expected_const.clone(),
                    expected_rust_places: expected_places(rust_crate, command),
                });
            }

            if let Some(rust_opcode) = opcode_by_name.get(expected_const.as_str())
                && *rust_opcode != st_opcode
            {
                opcode_value_mismatches.push(OpcodeValueMismatch {
                    st_command: command.name.clone(),
                    expected_rust_const: expected_const.clone(),
                    st_opcode,
                    rust_opcode: *rust_opcode,
                });
            }
        }

        let expected_fragments = expected_method_fragments(command);
        if has_obvious_impl_match(command, &expected_fragments, &command_identifiers) {
            obvious_command_impl_matches += 1;
        } else {
            missing_command_impls.push(MissingCommandImpl {
                st_command: command.name.clone(),
                opcode: command.opcode,
                expected_rust_method_fragments: expected_fragments,
                expected_rust_places: expected_places(rust_crate, command),
            });
        }
    }

    Ok(CoverageReport {
        firmware: spec.firmware.clone(),
        rust_crate: rust_crate.display().to_string(),
        commands_total: spec.commands.len(),
        rust_opcode_constants_total: opcodes.len(),
        covered_by_opcode,
        obvious_command_impl_matches,
        missing_opcode_constants,
        opcode_value_mismatches,
        missing_command_impls,
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

        if let Some((group, cgid)) = parse_group_cgid(line) {
            let _ = group;
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

fn load_command_identifiers(rust_crate: &Path) -> Result<HashMap<String, HashSet<String>>> {
    let command_dir = rust_crate.join("src/vendor/command");
    let mut by_group = HashMap::new();

    for group in ["gap", "gatt", "hal", "l2cap"] {
        let path = command_dir.join(format!("{group}.rs"));
        let source = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        by_group.insert(group.to_owned(), rust_identifiers(&source));
    }

    Ok(by_group)
}

fn rust_identifiers(source: &str) -> HashSet<String> {
    source
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|token| !token.is_empty())
        .map(normalize)
        .collect()
}

fn has_obvious_impl_match(
    command: &CommandSpec,
    expected_fragments: &[String],
    identifiers: &HashMap<String, HashSet<String>>,
) -> bool {
    let full = normalize(command.name.trim_start_matches("aci_"));
    identifiers.get(command.group.as_str()).is_some_and(|ids| {
        ids.contains(&full)
            || expected_fragments
                .iter()
                .map(|f| normalize(f))
                .any(|fragment| {
                    ids.contains(&fragment) || ids.iter().any(|id| id.ends_with(&fragment))
                })
    })
}

fn expected_rust_const(command: &CommandSpec) -> String {
    command.name.trim_start_matches("aci_").to_ascii_uppercase()
}

fn expected_method_fragments(command: &CommandSpec) -> Vec<String> {
    let without_aci = command.name.trim_start_matches("aci_");
    let mut base = without_aci
        .strip_prefix(&format!("{}_", command.group))
        .unwrap_or(without_aci);

    if command.group == "gatt" {
        base = base.strip_prefix("att_").unwrap_or(base);
    }

    alias_fragments(base)
}

fn alias_fragments(value: &str) -> Vec<String> {
    let aliases = value.split('_').map(part_aliases).collect::<Vec<_>>();
    let mut out = HashSet::new();
    build_alias_fragments(&aliases, 0, &mut Vec::new(), &mut out);
    let mut out = out.into_iter().collect::<Vec<_>>();
    out.sort();
    out
}

fn build_alias_fragments(
    aliases: &[Vec<String>],
    idx: usize,
    current: &mut Vec<String>,
    out: &mut HashSet<String>,
) {
    if idx == aliases.len() {
        out.insert(current.join("_"));
        return;
    }

    for alias in &aliases[idx] {
        current.push(alias.clone());
        build_alias_fragments(aliases, idx + 1, current, out);
        current.pop();
    }
}

fn part_aliases(part: &str) -> Vec<String> {
    match part {
        "addr" => strings(["addr", "address"]),
        "adv" => strings(["adv", "advertising"]),
        "auth" => strings(["auth", "authorization"]),
        "cfg" | "config" => vec![part.to_owned(), "configuration".to_owned()],
        "char" => strings(["char", "characteristic"]),
        "conn" => strings(["conn", "connection"]),
        "db" => strings(["db", "database"]),
        "desc" => strings(["desc", "descriptor"]),
        "del" => strings(["del", "delete"]),
        "disc" => strings(["disc", "discover"]),
        "establish" => strings(["establish", "establishment"]),
        "include" => strings(["include", "included"]),
        "info" => strings(["info", "information"]),
        "param" => strings(["param", "parameter"]),
        "proc" => strings(["proc", "procedure"]),
        "req" => strings(["req", "request"]),
        "resp" => strings(["resp", "response"]),
        "sec" => strings(["sec", "security"]),
        _ => vec![part.to_owned()],
    }
}

fn strings<const N: usize>(values: [&str; N]) -> Vec<String> {
    values.into_iter().map(str::to_owned).collect()
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

fn normalize(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}
