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

use cargo_metadata::MetadataCommand;
use proc_macro2::TokenStream;
use syn::parse::{Parse, ParseStream};
use syn::{Attribute, Ident, Item, LitInt, Token, braced, parenthesized};

use crate::FirmwareVersion;
use crate::model::{CoverageEntry, CoverageOrigin};
use crate::rust_cfg::attrs_active;

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
    // commands. These are public raw command declarations in the crate itself.
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

    let manifest_path = crate_dir.join("Cargo.toml");
    let metadata = MetadataCommand::new()
        .manifest_path(&manifest_path)
        .exec()
        .map_err(|error| {
            format!(
                "could not resolve dependencies from {} with cargo metadata: {error}",
                manifest_path.display()
            )
        })?;
    let package = metadata
        .packages
        .iter()
        .find(|package| package.manifest_path.as_std_path() == manifest_path)
        .or_else(|| {
            let canonical = manifest_path.canonicalize().ok()?;
            metadata.packages.iter().find(|package| {
                package
                    .manifest_path
                    .as_std_path()
                    .canonicalize()
                    .is_ok_and(|path| path == canonical)
            })
        })
        .ok_or_else(|| {
            format!(
                "cargo metadata did not return the package at {}",
                manifest_path.display()
            )
        })?;
    let resolve = metadata
        .resolve
        .as_ref()
        .ok_or_else(|| "cargo metadata did not return a dependency graph".to_owned())?;
    let node = resolve
        .nodes
        .iter()
        .find(|node| node.id == package.id)
        .ok_or_else(|| format!("cargo metadata has no dependency node for {}", package.name))?;
    let mut bt_hci_packages = node
        .deps
        .iter()
        .filter_map(|dependency| {
            metadata
                .packages
                .iter()
                .find(|candidate| candidate.id == dependency.pkg && candidate.name == "bt-hci")
        })
        .collect::<Vec<_>>();
    bt_hci_packages.sort_by_key(|package| package.id.to_string());
    bt_hci_packages.dedup_by_key(|package| package.id.clone());
    let [bt_hci] = bt_hci_packages.as_slice() else {
        return Err(format!(
            "{} must have exactly one direct bt-hci dependency; cargo metadata found {}",
            manifest_path.display(),
            bt_hci_packages.len()
        ));
    };
    let source = bt_hci
        .manifest_path
        .parent()
        .ok_or_else(|| format!("bt-hci manifest {} has no parent", bt_hci.manifest_path))?
        .as_std_path()
        .to_path_buf();
    if source.join("src/cmd.rs").is_file() && source.join("src/event.rs").is_file() {
        Ok(source)
    } else {
        Err(format!(
            "cargo metadata resolved bt-hci to {}, which is not a bt-hci source directory",
            source.display()
        ))
    }
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
        if !macro_name_is(&item.mac, "cmd")
            || (honor_cfg && !attrs_active(&item.attrs, firmware, path)?)
        {
            continue;
        }
        let header =
            syn::parse2::<CommandMacroHeader>(item.mac.tokens.clone()).map_err(|error| {
                format!(
                    "{}: could not parse cmd! declaration structurally: {error}",
                    path.display()
                )
            })?;
        let Some(ogf) = standard_ogf(&header.group) else {
            // `BASE` macro implementation details and non-standard groups are
            // deliberately not treated as public standard command declarations.
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
        let declarations =
            syn::parse2::<EventMacroDeclarations>(item.mac.tokens.clone()).map_err(|error| {
                format!(
                    "{}: could not parse {}! declarations structurally: {error}",
                    path.display(),
                    item.mac.path.segments.last().unwrap().ident
                )
            })?;
        for EventMacroHeader { name, code } in declarations.0 {
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

/// Small grammar for `[BASE] [attributes] Name(GROUP, OCF) { ... }`.
impl Parse for CommandMacroHeader {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        if input.peek(Ident) {
            let fork = input.fork();
            let marker = fork.parse::<Ident>()?;
            if marker == "BASE" {
                input.parse::<Ident>()?;
            }
        }
        let _attributes = input.call(Attribute::parse_outer)?;
        let name = input.parse::<Ident>()?;
        let arguments;
        parenthesized!(arguments in input);
        let group = arguments.parse::<Ident>()?;
        arguments.parse::<Token![,]>()?;
        let ocf = arguments.parse::<LitInt>()?.base10_parse::<u16>()?;
        if !arguments.is_empty() {
            return Err(arguments.error("unexpected command header tokens"));
        }
        let body;
        braced!(body in input);
        let _body = body.parse::<TokenStream>()?;
        if !input.is_empty() {
            return Err(input.error("unexpected tokens after cmd! declaration"));
        }
        Ok(Self {
            name: name.to_string(),
            group: group.to_string(),
            ocf,
        })
    }
}

struct EventMacroDeclarations(Vec<EventMacroHeader>);

struct EventMacroHeader {
    name: String,
    code: u16,
}

/// Small grammar for repeated `struct Name<'a>(CODE) { ... }` event records.
impl Parse for EventMacroDeclarations {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut events = Vec::new();
        while !input.is_empty() {
            let _attributes = input.call(Attribute::parse_outer)?;
            input.parse::<Token![struct]>()?;
            let name = input.parse::<Ident>()?;
            if input.peek(Token![<]) {
                input.parse::<syn::Generics>()?;
            }
            let code_group;
            parenthesized!(code_group in input);
            let code = code_group.parse::<LitInt>()?.base10_parse::<u16>()?;
            if !code_group.is_empty() {
                return Err(code_group.error("unexpected event code tokens"));
            }
            let body;
            braced!(body in input);
            let _body = body.parse::<TokenStream>()?;
            events.push(EventMacroHeader {
                name: name.to_string(),
                code,
            });
        }
        Ok(Self(events))
    }
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

fn sort_and_deduplicate(entries: &mut Vec<CoverageEntry>) {
    entries.sort_by_key(|entry| (entry.code, entry.name.clone()));
    entries.dedup_by(|left, right| left.code == right.code && left.name == right.name);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_standard_command_headers() {
        let header = syn::parse_str::<CommandMacroHeader>(
            "LeSetAdvData ( LE , 0x0008 ) { Params = [u8 ; 32] ; Return = () ; }",
        )
        .unwrap();
        assert_eq!(header.name, "LeSetAdvData");
        assert_eq!(header.group, "LE");
        assert_eq!(header.ocf, 8);
    }

    #[test]
    fn ignores_doc_attributes_before_a_command_header() {
        let header = syn::parse_str::<CommandMacroHeader>(
            "# [ doc = \"command\" ] LeTest ( LE , 0x001F ) { Params = () ; }",
        )
        .unwrap();
        assert_eq!(header.name, "LeTest");
    }

    #[test]
    fn parses_bt_hci_base_command_headers() {
        let header = syn::parse_str::<CommandMacroHeader>(
            "BASE # [ doc = \"command\" ] LeExtended ( LE , 0x0041 ) { Params = () ; }",
        )
        .unwrap();
        assert_eq!(header.name, "LeExtended");
        assert_eq!(header.ocf, 0x41);
    }

    #[test]
    fn parses_event_macro_headers_with_lifetimes() {
        let events = syn::parse_str::<EventMacroDeclarations>(
            "struct ConnectionComplete ( 0x03 ) { } struct LeAdvertisingReport < 'a > ( 0x02 ) { }",
        )
        .unwrap()
        .0
        .into_iter()
        .map(|event| (event.name, event.code))
        .collect::<Vec<_>>();
        assert_eq!(
            events,
            vec![
                ("ConnectionComplete".into(), 3),
                ("LeAdvertisingReport".into(), 2),
            ]
        );
    }
}
