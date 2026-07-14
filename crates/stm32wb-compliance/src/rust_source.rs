//! Feature-aware extraction of the crate's vendor command and event surface.
//!
//! The checker deliberately works from the Rust syntax tree rather than source
//! text. Command, event, and module cfgs are evaluated structurally for the
//! selected firmware.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use stm32wb_hci_schema::{
    Completion as SchemaCompletion, VendorCommand, VendorEvents as SchemaVendorEvents,
};
use syn::punctuated::Punctuated;
use syn::{Attribute, Expr, File, Item, ItemMacro, ItemMod, Lit, Meta, Path as SynPath};

use crate::FirmwareVersion;
use crate::envelope::WireEnvelope;
use crate::model::{CoverageEntry, CoverageOrigin, ProtocolCoverage};

pub(crate) struct CrateCoverage {
    pub(crate) descriptors: ProtocolCoverage,
    pub(crate) active_api: ProtocolCoverage,
    /// Command-envelope metadata declared by the active `vendor_cmd!` catalog.
    pub(crate) descriptor_metadata: BTreeMap<String, DescriptorMetadata>,
    /// Payload envelopes declared by the active `vendor_event!` catalog.
    pub(crate) event_metadata: BTreeMap<u16, EventMetadata>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DescriptorMetadata {
    pub(crate) name: String,
    pub(crate) code: u16,
    pub(crate) completion: DescriptorCompletion,
    /// Command parameter bytes, excluding the HCI command header.
    pub(crate) request: WireEnvelope,
    pub(crate) location: PathBuf,
}

/// Completion shape declared by one Rust `vendor_cmd!` invocation.
///
/// Command Complete owns its command return envelope; Command Status cannot
/// carry one, so invalid completion/return combinations are unrepresentable
/// after parsing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum DescriptorCompletion {
    CommandComplete { returns: WireEnvelope },
    CommandStatus,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EventMetadata {
    pub(crate) name: String,
    pub(crate) code: u16,
    /// Vendor event payload bytes, excluding the two-byte vendor event code.
    pub(crate) payload: WireEnvelope,
    pub(crate) location: PathBuf,
}

#[derive(Clone, Debug)]
struct DescriptorDefinition {
    name: String,
    code: u16,
    completion: DescriptorCompletion,
    request: WireEnvelope,
}

#[derive(Clone)]
struct SourceUnit {
    path: PathBuf,
    active: bool,
    file: File,
}

/// Load the declarative vendor command and event catalogs for one selected
/// firmware feature.
pub(crate) fn load_crate_coverage(
    crate_dir: &Path,
    firmware: FirmwareVersion,
) -> Result<CrateCoverage, String> {
    let command_root = crate_dir.join("src/vendor/command/mod.rs");
    let command_root_file = read_rust_file(&command_root)?;
    let mut command_sources = Vec::new();
    let mut visited = BTreeSet::new();
    collect_command_sources(
        command_root,
        command_root_file,
        true,
        firmware,
        &mut visited,
        &mut command_sources,
    )?;

    let descriptors = collect_descriptors(&command_sources, firmware)?;
    let active_commands = descriptor_coverage(&descriptors);

    let event_path = crate_dir.join("src/vendor/event/mod.rs");
    let event_file = read_rust_file(&event_path)?;
    let (active_events, event_metadata) =
        parse_vendor_event_declarations(&event_file, firmware, &event_path)?;

    let mut descriptor_coverage = ProtocolCoverage::default();
    let mut descriptor_metadata = BTreeMap::new();
    for descriptor in descriptors.values() {
        descriptor_coverage.commands.push(
            CoverageEntry::new(
                descriptor.code,
                &descriptor.name,
                CoverageOrigin::VendorCommandDescriptor,
            )
            .at(descriptor.location.clone()),
        );
        descriptor_metadata.insert(descriptor.name.clone(), descriptor.clone());
    }

    descriptor_coverage
        .commands
        .sort_by_key(|entry| (entry.code, entry.name.clone()));

    Ok(CrateCoverage {
        descriptors: descriptor_coverage,
        active_api: ProtocolCoverage {
            commands: active_commands,
            events: active_events,
        },
        descriptor_metadata,
        event_metadata,
    })
}

fn descriptor_coverage(descriptors: &BTreeMap<String, DescriptorMetadata>) -> Vec<CoverageEntry> {
    let mut commands = descriptors
        .values()
        .map(|descriptor| {
            CoverageEntry::new(
                descriptor.code,
                &descriptor.name,
                CoverageOrigin::VendorCommandDescriptor,
            )
            .at(descriptor.location.clone())
        })
        .collect::<Vec<_>>();
    commands.sort_by_key(|entry| (entry.code, entry.name.clone()));
    commands
}

fn read_rust_file(path: &Path) -> Result<File, String> {
    let source = fs::read_to_string(path)
        .map_err(|error| format!("could not read {}: {error}", path.display()))?;
    syn::parse_file(&source)
        .map_err(|error| format!("could not parse Rust source {}: {error}", path.display()))
}

/// Discover command modules from `src/vendor/command/mod.rs` rather than
/// maintaining a hand-written group list. This keeps a future, cfg-gated module
/// out of old firmware inventories and automatically brings it in when its cfg
/// becomes active.
fn collect_command_sources(
    path: PathBuf,
    file: File,
    inherited_active: bool,
    firmware: FirmwareVersion,
    visited: &mut BTreeSet<PathBuf>,
    sources: &mut Vec<SourceUnit>,
) -> Result<(), String> {
    let active = inherited_active && attrs_active(&file.attrs, firmware, &path)?;
    if !active {
        return Ok(());
    }

    let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
    if !visited.insert(canonical) {
        return Ok(());
    }

    // Descend before moving `file` into the source collection. Inline modules
    // are represented as their own source unit so direct-item scans never need
    // to recursively re-scan a parent module.
    for item in &file.items {
        let Item::Mod(module) = item else {
            continue;
        };
        let module_active = active && attrs_active(&module.attrs, firmware, &path)?;
        if !module_active {
            continue;
        }

        if let Some((_, items)) = &module.content {
            let inline_path = path.join(format!("<{}>", module.ident));
            collect_command_sources(
                inline_path,
                File {
                    shebang: None,
                    attrs: Vec::new(),
                    items: items.clone(),
                },
                module_active,
                firmware,
                visited,
                sources,
            )?;
        } else {
            let module_path = external_module_path(&path, module)?;
            let module_file = read_rust_file(&module_path)?;
            collect_command_sources(
                module_path,
                module_file,
                module_active,
                firmware,
                visited,
                sources,
            )?;
        }
    }

    sources.push(SourceUnit { path, active, file });
    Ok(())
}

fn external_module_path(parent_path: &Path, module: &ItemMod) -> Result<PathBuf, String> {
    if let Some(path) = module_path_override(module)? {
        return Ok(parent_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(path));
    }

    let parent_dir = parent_path.parent().unwrap_or_else(|| Path::new("."));
    let child_dir = if parent_path.file_name().is_some_and(|name| name == "mod.rs") {
        parent_dir.to_path_buf()
    } else {
        parent_dir.join(
            parent_path
                .file_stem()
                .ok_or_else(|| format!("{} has no file stem", parent_path.display()))?,
        )
    };
    let flat = child_dir.join(format!("{}.rs", module.ident));
    let nested = child_dir.join(module.ident.to_string()).join("mod.rs");

    match (flat.is_file(), nested.is_file()) {
        (true, false) => Ok(flat),
        (false, true) => Ok(nested),
        (false, false) => Err(format!(
            "{}: active module `{}` has no source file (looked for {} or {})",
            parent_path.display(),
            module.ident,
            flat.display(),
            nested.display(),
        )),
        (true, true) => Err(format!(
            "{}: active module `{}` is ambiguous: both {} and {} exist",
            parent_path.display(),
            module.ident,
            flat.display(),
            nested.display(),
        )),
    }
}

fn module_path_override(module: &ItemMod) -> Result<Option<String>, String> {
    let Some(attribute) = module
        .attrs
        .iter()
        .find(|attribute| attribute.path().is_ident("path"))
    else {
        return Ok(None);
    };
    let Meta::NameValue(value) = &attribute.meta else {
        return Err(format!(
            "module `{}`: unsupported #[path] attribute shape",
            module.ident
        ));
    };
    let Expr::Lit(literal) = &value.value else {
        return Err(format!(
            "module `{}`: #[path] must contain a string literal",
            module.ident
        ));
    };
    let Lit::Str(path) = &literal.lit else {
        return Err(format!(
            "module `{}`: #[path] must contain a string literal",
            module.ident
        ));
    };
    Ok(Some(path.value()))
}

fn collect_descriptors(
    sources: &[SourceUnit],
    firmware: FirmwareVersion,
) -> Result<BTreeMap<String, DescriptorMetadata>, String> {
    let mut descriptors = BTreeMap::<String, DescriptorMetadata>::new();
    let mut codes = BTreeMap::<u16, DescriptorMetadata>::new();

    for source in sources {
        if !source.active {
            continue;
        }
        for item in &source.file.items {
            let Item::Macro(item) = item else {
                continue;
            };
            if !is_macro_named(&item.mac.path, "vendor_cmd")
                || !attrs_active(&item.attrs, firmware, &source.path)?
            {
                continue;
            }

            let definition = parse_vendor_descriptor(item, &source.path)?;
            let metadata = DescriptorMetadata {
                name: definition.name.clone(),
                code: definition.code,
                completion: definition.completion,
                request: definition.request,
                location: source.path.clone(),
            };

            if let Some(previous) = descriptors.get(&definition.name) {
                return Err(format!(
                    "{}: descriptor `{}` is active more than once (also declared in {})",
                    source.path.display(),
                    definition.name,
                    previous.location.display()
                ));
            }
            if let Some(previous) = codes.insert(definition.code, metadata.clone()) {
                return Err(format!(
                    "{}: descriptors `{}` and `{}` both declare active vendor OCF 0x{:03X}",
                    source.path.display(),
                    previous.name,
                    definition.name,
                    definition.code,
                ));
            }
            descriptors.insert(definition.name, metadata);
        }
    }

    if descriptors.is_empty() {
        return Err("no active vendor_cmd! command descriptors were found".to_owned());
    }
    Ok(descriptors)
}

fn parse_vendor_descriptor(item: &ItemMacro, path: &Path) -> Result<DescriptorDefinition, String> {
    let command = syn::parse2::<VendorCommand>(item.mac.tokens.clone()).map_err(|error| {
        format!(
            "{}: unsupported vendor_cmd! declaration: {error}",
            path.display()
        )
    })?;

    let request = wire_envelope(
        command.params.min_len(),
        command.params.max_len().min(usize::from(u8::MAX)),
    );
    let completion = match command.completion {
        SchemaCompletion::CommandComplete => {
            let returns = command
                .returns
                .as_ref()
                .expect("the shared parser requires Return for CommandComplete");
            DescriptorCompletion::CommandComplete {
                returns: wire_envelope(returns.min_len(), returns.max_len()),
            }
        }
        SchemaCompletion::CommandStatus => DescriptorCompletion::CommandStatus,
    };

    Ok(DescriptorDefinition {
        name: command.name.to_string(),
        code: command.ocf(),
        completion,
        request,
    })
}

fn wire_envelope(minimum: usize, maximum: usize) -> WireEnvelope {
    if minimum == maximum {
        WireEnvelope::fixed(maximum)
    } else {
        WireEnvelope::bounded(minimum, maximum)
    }
}

fn parse_vendor_event_declarations(
    file: &File,
    firmware: FirmwareVersion,
    path: &Path,
) -> Result<(Vec<CoverageEntry>, BTreeMap<u16, EventMetadata>), String> {
    if !attrs_active(&file.attrs, firmware, path)? {
        return Err(format!(
            "{}: VendorEvent source is disabled for selected firmware {firmware}",
            path.display()
        ));
    }

    let mut macros = Vec::new();
    collect_vendor_event_macros(&file.items, firmware, path, &mut macros)?;
    let [item] = macros.as_slice() else {
        return Err(format!(
            "{}: found {} active `vendor_event!` catalogs; expected exactly one",
            path.display(),
            macros.len()
        ));
    };
    let invocation =
        syn::parse2::<SchemaVendorEvents>(item.mac.tokens.clone()).map_err(|error| {
            format!(
                "{}: unsupported vendor_event! declaration: {error}",
                path.display()
            )
        })?;

    let mut events = Vec::new();
    let mut metadata = BTreeMap::new();
    for definition in invocation.events {
        if !attrs_active(&definition.attrs, firmware, path)? {
            continue;
        }
        let event = EventMetadata {
            name: definition.name.to_string(),
            code: definition.code,
            payload: wire_envelope(definition.payload.min_len(), definition.payload.max_len()),
            location: path.to_path_buf(),
        };
        metadata.insert(event.code, event.clone());
        events.push(
            CoverageEntry::new(event.code, &event.name, CoverageOrigin::VendorEventDispatch)
                .at(path.to_path_buf()),
        );
    }
    if events.is_empty() {
        return Err(format!(
            "{}: vendor_event! has no active declarations for firmware {firmware}",
            path.display()
        ));
    }
    events.sort_by_key(|entry| (entry.code, entry.name.clone()));
    Ok((events, metadata))
}

fn collect_vendor_event_macros<'ast>(
    items: &'ast [Item],
    firmware: FirmwareVersion,
    path: &Path,
    macros: &mut Vec<&'ast ItemMacro>,
) -> Result<(), String> {
    for item in items {
        if !item_is_active(item, firmware, path)? {
            continue;
        }
        match item {
            Item::Macro(item) if is_macro_named(&item.mac.path, "vendor_event") => {
                macros.push(item);
            }
            Item::Mod(module) if module.content.is_some() => {
                let (_, nested) = module.content.as_ref().expect("checked above");
                collect_vendor_event_macros(nested, firmware, path, macros)?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn item_is_active(item: &Item, firmware: FirmwareVersion, path: &Path) -> Result<bool, String> {
    let attributes = match item {
        Item::Const(item) => &item.attrs,
        Item::Enum(item) => &item.attrs,
        Item::ExternCrate(item) => &item.attrs,
        Item::Fn(item) => &item.attrs,
        Item::ForeignMod(item) => &item.attrs,
        Item::Impl(item) => &item.attrs,
        Item::Macro(item) => &item.attrs,
        Item::Mod(item) => &item.attrs,
        Item::Static(item) => &item.attrs,
        Item::Struct(item) => &item.attrs,
        Item::Trait(item) => &item.attrs,
        Item::TraitAlias(item) => &item.attrs,
        Item::Type(item) => &item.attrs,
        Item::Union(item) => &item.attrs,
        Item::Use(item) => &item.attrs,
        Item::Verbatim(_) => return Ok(true),
        _ => return Ok(true),
    };
    attrs_active(attributes, firmware, path)
}

/// Evaluate all `#[cfg]` / `#[cfg_attr]` attributes attached to one syntax
/// node. Unknown predicates are errors rather than false: silently treating an
/// unsupported condition as disabled would turn an incomplete parser into a
/// false compliance success.
fn attrs_active(
    attributes: &[Attribute],
    firmware: FirmwareVersion,
    path: &Path,
) -> Result<bool, String> {
    let mut active = true;
    for attribute in attributes {
        if attribute.path().is_ident("cfg") {
            active &= eval_cfg_attribute(&attribute.meta, firmware, path)?;
        } else if attribute.path().is_ident("cfg_attr") {
            active &= eval_cfg_attr_attribute(&attribute.meta, firmware, path)?;
        }
    }
    Ok(active)
}

fn eval_cfg_attribute(meta: &Meta, firmware: FirmwareVersion, path: &Path) -> Result<bool, String> {
    let Meta::List(list) = meta else {
        return Err(format!("{}: #[cfg] must use parentheses", path.display()));
    };
    let conditions = list
        .parse_args_with(Punctuated::<Meta, syn::Token![,]>::parse_terminated)
        .map_err(|error| {
            format!(
                "{}: could not parse #[cfg(...)] condition: {error}",
                path.display()
            )
        })?;
    let conditions = conditions.into_iter().collect::<Vec<_>>();
    let [condition] = conditions.as_slice() else {
        return Err(format!(
            "{}: #[cfg(...)] requires exactly one condition",
            path.display()
        ));
    };
    eval_cfg_meta(condition, firmware, path)
}

fn eval_cfg_attr_attribute(
    meta: &Meta,
    firmware: FirmwareVersion,
    path: &Path,
) -> Result<bool, String> {
    let Meta::List(list) = meta else {
        return Err(format!(
            "{}: #[cfg_attr] must use parentheses",
            path.display()
        ));
    };
    let values = list
        .parse_args_with(Punctuated::<Meta, syn::Token![,]>::parse_terminated)
        .map_err(|error| {
            format!(
                "{}: could not parse #[cfg_attr(...)] condition: {error}",
                path.display()
            )
        })?;
    let mut values = values.into_iter();
    let Some(condition) = values.next() else {
        return Err(format!("{}: #[cfg_attr] has no condition", path.display()));
    };
    if !eval_cfg_meta(&condition, firmware, path)? {
        return Ok(true);
    }
    let mut active = true;
    for generated in values {
        active &= eval_generated_cfg_attribute(&generated, firmware, path)?;
    }
    Ok(active)
}

fn eval_generated_cfg_attribute(
    generated: &Meta,
    firmware: FirmwareVersion,
    path: &Path,
) -> Result<bool, String> {
    if generated.path().is_ident("cfg") {
        return eval_cfg_attribute(generated, firmware, path);
    }
    if generated.path().is_ident("cfg_attr") {
        return eval_cfg_attr_attribute(generated, firmware, path);
    }
    Ok(true)
}

fn eval_cfg_meta(meta: &Meta, firmware: FirmwareVersion, path: &Path) -> Result<bool, String> {
    match meta {
        Meta::Path(path_meta) => {
            let name = path_meta
                .get_ident()
                .map(ToString::to_string)
                .ok_or_else(|| {
                    format!(
                        "{}: unsupported multi-segment cfg path `{}`",
                        path.display(),
                        cfg_path_name(path_meta)
                    )
                })?;
            if let Some(value) = firmware.matches_version_cfg(&name) {
                return Ok(value);
            }
            match name.as_str() {
                // The checker invokes `cargo check`, not `cargo test` or rustdoc.
                "test" | "doctest" | "doc" => Ok(false),
                // `cargo check` uses the development profile unless the caller
                // explicitly changes it, matching this conservative default.
                "debug_assertions" => Ok(true),
                _ => Err(format!(
                    "{}: unsupported cfg predicate `{name}`; add it to the compliance cfg evaluator",
                    path.display()
                )),
            }
        }
        Meta::NameValue(value) if value.path.is_ident("feature") => {
            let Expr::Lit(literal) = &value.value else {
                return Err(format!(
                    "{}: cfg(feature = ...) must use a string literal",
                    path.display()
                ));
            };
            let Lit::Str(feature) = &literal.lit else {
                return Err(format!(
                    "{}: cfg(feature = ...) must use a string literal",
                    path.display()
                ));
            };
            Ok(feature.value() == firmware.feature_name())
        }
        Meta::NameValue(value) => Err(format!(
            "{}: unsupported cfg key `{}`",
            path.display(),
            cfg_path_name(&value.path)
        )),
        Meta::List(list) if list.path.is_ident("all") => {
            let values = parse_cfg_list(list, path)?;
            values
                .iter()
                .map(|value| eval_cfg_meta(value, firmware, path))
                .try_fold(true, |active, value| value.map(|value| active && value))
        }
        Meta::List(list) if list.path.is_ident("any") => {
            let values = parse_cfg_list(list, path)?;
            values
                .iter()
                .map(|value| eval_cfg_meta(value, firmware, path))
                .try_fold(false, |active, value| value.map(|value| active || value))
        }
        Meta::List(list) if list.path.is_ident("not") => {
            let values = parse_cfg_list(list, path)?;
            let values = values.iter().collect::<Vec<_>>();
            let [value] = values.as_slice() else {
                return Err(format!(
                    "{}: cfg(not(...)) requires exactly one predicate",
                    path.display()
                ));
            };
            Ok(!eval_cfg_meta(value, firmware, path)?)
        }
        Meta::List(list) => Err(format!(
            "{}: unsupported cfg combinator `{}`",
            path.display(),
            cfg_path_name(&list.path)
        )),
    }
}

fn parse_cfg_list(
    list: &syn::MetaList,
    path: &Path,
) -> Result<Punctuated<Meta, syn::Token![,]>, String> {
    list.parse_args_with(Punctuated::<Meta, syn::Token![,]>::parse_terminated)
        .map_err(|error| format!("{}: could not parse cfg list: {error}", path.display()))
}

fn is_macro_named(path: &SynPath, name: &str) -> bool {
    path.segments
        .last()
        .is_some_and(|segment| segment.ident == name)
}

fn cfg_path_name(path: &SynPath) -> String {
    path.segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn version(major: u16, minor: u16, patch: u16) -> FirmwareVersion {
        FirmwareVersion::new(major, minor, patch)
    }

    fn fixture_descriptors(
        source: &str,
        firmware: FirmwareVersion,
    ) -> (Vec<CoverageEntry>, BTreeMap<String, DescriptorMetadata>) {
        let path = PathBuf::from("fixture.rs");
        let unit = SourceUnit {
            path: path.clone(),
            active: true,
            file: syn::parse_file(source).unwrap(),
        };
        let descriptors = collect_descriptors(std::slice::from_ref(&unit), firmware).unwrap();
        let commands = descriptor_coverage(&descriptors);
        (commands, descriptors)
    }

    #[test]
    fn evaluates_nested_firmware_cfgs() {
        let firmware = version(0, 17, 0);
        let path = Path::new("fixture.rs");
        let attribute: Attribute = syn::parse_quote!(
            #[cfg(all(since_fw_0_17_0, not(since_fw_0_17_1)))]
        );
        assert!(attrs_active(&[attribute], firmware, path).unwrap());
    }

    #[test]
    fn keeps_descriptor_return_metadata() {
        let source = r#"
            pub trait Commands { async fn command(&self); }
            vendor_cmd! {
                Current(cgid = 0x0, cid = 0x03) {
                    Params = ();
                    Completion = CommandComplete;
                    Return = Result { value: [u8; 8] => 8, };
                }
            }
            impl<T> Commands for T {
                hci_impl_params!(command, Params, Current);
            }
        "#;
        let (_, descriptors) = fixture_descriptors(source, version(0, 17, 0));
        let descriptor = descriptors.get("Current").unwrap();
        assert_eq!(descriptor.code, 0x0003);
        assert_eq!(
            descriptor.completion,
            DescriptorCompletion::CommandComplete {
                returns: WireEnvelope::fixed(8),
            }
        );
        assert_eq!(descriptor.request, WireEnvelope::fixed(0));
    }

    #[test]
    fn parses_declarative_fixed_command_shapes() {
        let source = r#"
            pub trait Commands { async fn command(&self); }
            vendor_cmd! {
                Current(cgid = 0x0, cid = 0x03) {
                    Params = {
                        role: Role => 1,
                        enabled: bool => 1,
                        name_len: u8 => 1,
                    };
                    Completion = CommandComplete;
                    Return = Result {
                        first_handle: AttributeHandle => 2,
                        second_handle: AttributeHandle => 2,
                        third_handle: AttributeHandle => 2,
                    };
                }
            }
            impl<T> Commands for T {
                async fn command(&self) { Current::try_new(); }
            }
        "#;
        let (_, descriptors) = fixture_descriptors(source, version(0, 17, 0));
        let descriptor = descriptors.get("Current").unwrap();
        assert_eq!(
            descriptor.completion,
            DescriptorCompletion::CommandComplete {
                returns: WireEnvelope::fixed(6),
            }
        );
        assert_eq!(descriptor.request, WireEnvelope::fixed(3));
    }

    #[test]
    fn parses_the_qualified_proc_macro_with_the_same_descriptor_contract() {
        let source = r#"
            stm32wb_hci_macros::vendor_cmd! {
                GapSetIoCapability(cgid = 0x1, cid = 0x05) {
                    Params = { io_capability: IoCapability => 1, };
                    Completion = CommandComplete;
                    Return = ();
                }
            }
        "#;
        let (_, descriptors) = fixture_descriptors(source, version(0, 17, 1));
        let descriptor = descriptors.get("GapSetIoCapability").unwrap();
        assert_eq!(descriptor.code, 0x085);
        assert_eq!(descriptor.request, WireEnvelope::fixed(1));
        assert_eq!(
            descriptor.completion,
            DescriptorCompletion::CommandComplete {
                returns: WireEnvelope::fixed(0),
            }
        );
    }

    #[test]
    fn parses_explicit_command_status_shape() {
        let source = r#"
            pub trait Commands { async fn command(&self); }
            vendor_cmd! {
                Current(cgid = 0x0, cid = 0x03) {
                    Params = { procedure: u8 => 1, };
                    Completion = CommandStatus;
                }
            }
            impl<T> Commands for T {
                async fn command(&self) { Current::new(); }
            }
        "#;
        let (_, descriptors) = fixture_descriptors(source, version(0, 17, 0));
        let descriptor = descriptors.get("Current").unwrap();
        assert_eq!(descriptor.completion, DescriptorCompletion::CommandStatus);
        assert_eq!(descriptor.request, WireEnvelope::fixed(1));
    }

    #[test]
    fn parses_counted_request_and_bounded_return_shapes() {
        let source = r#"
            pub trait Commands { async fn command(&self); }
            vendor_cmd! {
                Current(cgid = 0x0, cid = 0x03) {
                    Params<'a> = {
                        conn_handle: ConnHandle => 2,
                        handles: &'a [AttributeHandle] => {
                            kind: counted_items,
                            count: u8 => 1,
                            item: AttributeHandle => 2,
                            max_items: 126,
                        },
                    };
                    Completion = CommandComplete;
                    Return = Result {
                        total_length: u16 => 2,
                        value: BoundedBytes<249> => {
                            kind: counted_bytes,
                            count: u16 => 2,
                            max_len: 249,
                        },
                    };
                }
            }
            impl<T> Commands for T {
                async fn command(&self) { Current::new(); }
            }
        "#;
        let (_, descriptors) = fixture_descriptors(source, version(0, 17, 0));
        let descriptor = descriptors.get("Current").unwrap();
        assert_eq!(
            descriptor.completion,
            DescriptorCompletion::CommandComplete {
                returns: WireEnvelope::bounded(4, 253),
            }
        );
        assert_eq!(descriptor.request, WireEnvelope::bounded(3, 255));
    }

    #[test]
    fn parses_inline_trailing_byte_returns() {
        let source = r#"
            pub trait Commands { async fn command(&self); }
            vendor_cmd! {
                Current(cgid = 0x0, cid = 0x03) {
                    Params = { offset: u8 => 1, };
                    Completion = CommandComplete;
                    Return = Result {
                        value: BoundedBytes<16> => {
                            kind: trailing_bytes,
                            min_len: 1,
                            max_len: 16,
                        },
                    };
                }
            }
            impl<T> Commands for T {
                async fn command(&self) { Current::new(); }
            }
        "#;
        let (_, descriptors) = fixture_descriptors(source, version(0, 17, 0));
        let descriptor = descriptors.get("Current").unwrap();
        assert_eq!(descriptor.request, WireEnvelope::fixed(1));
        assert_eq!(
            descriptor.completion,
            DescriptorCompletion::CommandComplete {
                returns: WireEnvelope::bounded(1, 16),
            }
        );
    }

    #[test]
    fn rejects_fields_after_trailing_bytes() {
        let source = r#"
            vendor_cmd! {
                Current(cgid = 0x0, cid = 0x03) {
                    Params<'a> = {
                        value: &'a [u8] => {
                            kind: trailing_bytes,
                            min_len: 0,
                            max_len: 16,
                        },
                        suffix: u8 => 1,
                    };
                    Completion = CommandStatus;
                }
            }
        "#;
        let file = syn::parse_file(source).unwrap();
        let Item::Macro(item) = &file.items[0] else {
            panic!("expected vendor_cmd! macro item");
        };
        let error = parse_vendor_descriptor(item, Path::new("fixture.rs")).unwrap_err();
        assert!(error.contains("trailing_bytes must be the final declarative field"));
    }

    #[test]
    fn rejects_invalid_or_legacy_command_ids() {
        for (source, expected) in [
            (
                "vendor_cmd! { Current(cgid = 0x8, cid = 0x01) {} }",
                "command group ID must fit in three bits",
            ),
            (
                "vendor_cmd! { Current(cgid = 0x1, cid = 0x80) {} }",
                "command ID must fit in seven bits",
            ),
            ("vendor_cmd! { Current(CURRENT) {} }", "expected `cgid`"),
        ] {
            let file = syn::parse_file(source).unwrap();
            let Item::Macro(item) = &file.items[0] else {
                panic!("expected vendor_cmd! macro item");
            };
            let error = parse_vendor_descriptor(item, Path::new("fixture.rs")).unwrap_err();
            assert!(error.contains(expected), "{error}");
        }

        let unit = SourceUnit {
            path: PathBuf::from("fixture.rs"),
            active: true,
            file: syn::parse_file(
                r#"
                    vendor_cmd! { First(cgid = 0x1, cid = 0x02) { Params = (); Completion = CommandStatus; } }
                    vendor_cmd! { Second(cgid = 0x1, cid = 0x02) { Params = (); Completion = CommandStatus; } }
                "#,
            )
            .unwrap(),
        };
        let error =
            collect_descriptors(std::slice::from_ref(&unit), version(0, 17, 0)).unwrap_err();
        assert!(error.contains("both declare active vendor OCF 0x082"));
    }

    #[test]
    fn parses_tagged_and_bitmap_selected_request_shapes() {
        let source = r#"
            pub trait Commands { async fn command(&self); }
            vendor_cmd! {
                Current(cgid = 0x0, cid = 0x03) {
                    Params<'a> = {
                        scanning_phys: u8 => 1,
                        phy_params: &'a [Phy] => {
                            kind: bitmap_items,
                            bitmap: scanning_phys,
                            mask: 0x05,
                            item: Phy => 5,
                            max_items: 2,
                        },
                        uuid: &'a Uuid => {
                            kind: tagged,
                            tag: u8 => 1,
                            variants: {
                                Uuid::Uuid16(value) => {
                                    tag: 0x01,
                                    fields: { value: u16 => 2, },
                                },
                                Uuid::Uuid128(value) => {
                                    tag: 0x02,
                                    fields: { value: [u8; 16] => 16, },
                                },
                            },
                            min_len: 3,
                            max_len: 17,
                        },
                    };
                    Completion = CommandStatus;
                }
            }
            impl<T> Commands for T {
                async fn command(&self) { Current::try_new(); }
            }
        "#;
        let (_, descriptors) = fixture_descriptors(source, version(0, 17, 0));
        let descriptor = descriptors.get("Current").unwrap();
        assert_eq!(descriptor.completion, DescriptorCompletion::CommandStatus);
        assert_eq!(descriptor.request, WireEnvelope::bounded(4, 28));
    }

    #[test]
    fn parses_constraints_and_rejects_unknown_parameter_references() {
        let source = r#"
            vendor_cmd! {
                Current(cgid = 0x0, cid = 0x03) {
                    Params<'a> = {
                        minimum: u16 => 2,
                        maximum: u16 => 2,
                        mode: u8 => 1,
                        data: &'a [u8] => {
                            kind: counted_bytes,
                            count: u8 => 1,
                            max_len: 16,
                        },
                    };
                    Constraints = {
                        ordered(minimum, maximum);
                        range(minimum, 0x20, 0x4000);
                        one_of(mode, [0x00, 0x02]);
                        one_of_or_range(maximum, [0], 0x20, 0x4000);
                        paired_value(minimum, maximum, 0);
                        ordered_when_in_range(minimum, maximum, 0x20, 0x4000);
                        implies_eq(mode, 0x00, maximum, 0);
                        implies_range(mode, 0x02, maximum, 0x20, 0x4000);
                        pawr_subevents_fit(minimum, mode, maximum);
                        pawr_response_slots_fit(mode, minimum, maximum, mode, minimum);
                        len_at_most(data, mode);
                    };
                    Completion = CommandStatus;
                }
            }
        "#;
        let (_, descriptors) = fixture_descriptors(source, version(0, 17, 0));
        assert_eq!(
            descriptors.get("Current").unwrap().request,
            WireEnvelope::bounded(6, 22)
        );

        let source = r#"
            vendor_cmd! {
                Current(cgid = 0x0, cid = 0x03) {
                    Params = { value: u8 => 1, };
                    Constraints = { ordered(value, missing); };
                    Completion = CommandStatus;
                }
            }
        "#;
        let file = syn::parse_file(source).unwrap();
        let Item::Macro(item) = &file.items[0] else {
            panic!("expected vendor_cmd! macro item");
        };
        let error = parse_vendor_descriptor(item, Path::new("fixture.rs")).unwrap_err();
        assert!(error.contains("unknown parameter(s): missing"), "{error}");
    }

    #[test]
    fn rejects_removed_command_payload_fields() {
        for (source, section) in [
            (
                r#"
                    vendor_cmd! {
                        Current(cgid = 0x0, cid = 0x03) {
                            Params<'a> = {
                                uuid: &'a Uuid => {
                                    kind: payload,
                                    min_len: 3,
                                    max_len: 17,
                                },
                            };
                            Completion = CommandStatus;
                        }
                    }
                "#,
                "Params",
            ),
            (
                r#"
                    vendor_cmd! {
                        Current(cgid = 0x0, cid = 0x03) {
                            Params = ();
                            Completion = CommandComplete;
                            Return = Result {
                                uuid: Uuid => {
                                    kind: payload,
                                    min_len: 3,
                                    max_len: 17,
                                },
                            };
                        }
                    }
                "#,
                "Return",
            ),
        ] {
            let file = syn::parse_file(source).unwrap();
            let Item::Macro(item) = &file.items[0] else {
                panic!("expected vendor_cmd! macro item");
            };
            let error = parse_vendor_descriptor(item, Path::new("fixture.rs")).unwrap_err();
            assert!(error.contains("removed `kind: payload`"), "{error}");
            assert!(error.contains(section), "{error}");
            assert!(error.contains("inline the wire schema"), "{error}");
        }
    }

    #[test]
    fn rejects_incorrect_tagged_range_and_bitmap_cardinality() {
        let bad_tagged = r#"
            vendor_cmd! {
                Current(cgid = 0x0, cid = 0x03) {
                    Params<'a> = {
                        uuid: &'a Uuid => {
                            kind: tagged,
                            tag: u8 => 1,
                            variants: {
                                Uuid::Uuid16(value) => {
                                    tag: 0x01,
                                    fields: { value: u16 => 2, },
                                },
                                Uuid::Uuid128(value) => {
                                    tag: 0x02,
                                    fields: { value: [u8; 16] => 16, },
                                },
                            },
                            min_len: 2,
                            max_len: 17,
                        },
                    };
                    Completion = CommandStatus;
                }
            }
        "#;
        let file = syn::parse_file(bad_tagged).unwrap();
        let Item::Macro(item) = &file.items[0] else {
            panic!("expected vendor_cmd! macro item");
        };
        let error = parse_vendor_descriptor(item, Path::new("fixture.rs")).unwrap_err();
        assert!(error.contains("variants require 3..=17"));

        let bad_bitmap = r#"
            vendor_cmd! {
                Current(cgid = 0x0, cid = 0x03) {
                    Params<'a> = {
                        scanning_phys: u8 => 1,
                        phy_params: &'a [Phy] => {
                            kind: bitmap_items,
                            bitmap: scanning_phys,
                            mask: 0x05,
                            item: Phy => 5,
                            max_items: 3,
                        },
                    };
                    Completion = CommandStatus;
                }
            }
        "#;
        let file = syn::parse_file(bad_bitmap).unwrap();
        let Item::Macro(item) = &file.items[0] else {
            panic!("expected vendor_cmd! macro item");
        };
        let error = parse_vendor_descriptor(item, Path::new("fixture.rs")).unwrap_err();
        assert!(error.contains("mask selects 2 bits but max_items is 3"));
    }

    #[test]
    fn rejects_tagged_payload_field_not_bound_by_pattern() {
        let source = r#"
            vendor_cmd! {
                Current(cgid = 0x0, cid = 0x03) {
                    Params<'a> = {
                        uuid: &'a Uuid => {
                            kind: tagged,
                            tag: u8 => 1,
                            variants: {
                                Uuid::Uuid16(actual) => {
                                    tag: 0x01,
                                    fields: { typo: u16 => 2, },
                                },
                            },
                            min_len: 3,
                            max_len: 3,
                        },
                    };
                    Completion = CommandStatus;
                }
            }
        "#;
        let file = syn::parse_file(source).unwrap();
        let Item::Macro(item) = &file.items[0] else {
            panic!("expected vendor_cmd! macro item");
        };
        let error = parse_vendor_descriptor(item, Path::new("fixture.rs")).unwrap_err();
        assert!(error.contains("payload field `typo` is not bound"));
    }

    #[test]
    fn rejects_return_on_explicit_command_status() {
        let source = r#"
            vendor_cmd! {
                Current(cgid = 0x0, cid = 0x03) {
                    Params = ();
                    Completion = CommandStatus;
                    Return = ();
                }
            }
        "#;
        let file = syn::parse_file(source).unwrap();
        let Item::Macro(item) = &file.items[0] else {
            panic!("expected vendor_cmd! macro item");
        };
        let error = parse_vendor_descriptor(item, Path::new("fixture.rs")).unwrap_err();
        assert!(error.contains("CommandStatus and must not declare Return"));
    }

    #[test]
    fn rejects_legacy_completion_inference_and_return_buffers() {
        let missing_completion = syn::parse_file(
            r#"vendor_cmd! { Current(cgid = 0x0, cid = 0x03) { Params = (); Return = (); } }"#,
        )
        .unwrap();
        let Item::Macro(item) = &missing_completion.items[0] else {
            panic!("expected vendor_cmd! macro item");
        };
        let error = parse_vendor_descriptor(item, Path::new("fixture.rs")).unwrap_err();
        assert!(error.contains("missing a `Completion = ...` declaration"));

        let return_buffer = syn::parse_file(
            r#"
                vendor_cmd! {
                    Current(cgid = 0x0, cid = 0x03) {
                        Params = ();
                        Completion = CommandComplete;
                        Return = ReturnBuffer<9>;
                    }
                }
            "#,
        )
        .unwrap();
        let Item::Macro(item) = &return_buffer.items[0] else {
            panic!("expected vendor_cmd! macro item");
        };
        let error = parse_vendor_descriptor(item, Path::new("fixture.rs")).unwrap_err();
        assert!(error.contains("expected `()` or an inline named field body"));
    }

    #[test]
    fn loads_declarative_variable_shapes_from_the_real_crate() {
        let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../stm32wb-hci");
        let coverage = load_crate_coverage(&crate_dir, version(0, 17, 1)).unwrap();

        let update = coverage
            .descriptor_metadata
            .get("GapUpdateAdvertisingData")
            .unwrap();
        assert_eq!(update.request, WireEnvelope::bounded(1, 32));
        assert_eq!(
            update.completion,
            DescriptorCompletion::CommandComplete {
                returns: WireEnvelope::fixed(0),
            }
        );

        let discoverable = coverage
            .descriptor_metadata
            .get("GapSetLimitedDiscoverable")
            .unwrap();
        // Independent field capacities exceed one HCI command, but the
        // generated constructor rejects aggregate payloads above 255 bytes.
        assert_eq!(discoverable.request, WireEnvelope::bounded(13, 255));

        let read = coverage
            .descriptor_metadata
            .get("GattReadHandleValue")
            .unwrap();
        assert_eq!(read.request, WireEnvelope::fixed(6));
        assert_eq!(
            read.completion,
            DescriptorCompletion::CommandComplete {
                returns: WireEnvelope::bounded(4, 251),
            }
        );

        assert!(
            coverage
                .descriptor_metadata
                .contains_key("GattReadMultipleVarCharValue")
        );

        let tagged = coverage
            .descriptor_metadata
            .get("GattDiscoverPrimaryServicesByUUID")
            .unwrap();
        assert_eq!(tagged.request, WireEnvelope::bounded(5, 19));
        assert_eq!(tagged.completion, DescriptorCompletion::CommandStatus);

        let bitmap = coverage.descriptor_metadata.get("GapExtStartScan").unwrap();
        assert_eq!(bitmap.request, WireEnvelope::bounded(10, 20));
        assert_eq!(bitmap.completion, DescriptorCompletion::CommandStatus);

        let bonded = coverage
            .descriptor_metadata
            .get("GapGetBondedDevices")
            .unwrap();
        // Count plus at most 35 seven-byte address records; status is framing.
        assert_eq!(
            bonded.completion,
            DescriptorCompletion::CommandComplete {
                returns: WireEnvelope::bounded(1, 246),
            }
        );

        let config = coverage
            .descriptor_metadata
            .get("HalReadConfigData")
            .unwrap();
        assert_eq!(
            config.completion,
            DescriptorCompletion::CommandComplete {
                returns: WireEnvelope::bounded(1, 16),
            }
        );

        let channels = coverage
            .descriptor_metadata
            .get("L2CocConnectConfirm")
            .unwrap();
        assert_eq!(
            channels.completion,
            DescriptorCompletion::CommandComplete {
                returns: WireEnvelope::bounded(1, 6),
            }
        );

        assert_eq!(coverage.event_metadata.len(), 56);
        let gap_procedure = coverage.event_metadata.get(&0x0407).unwrap();
        assert_eq!(gap_procedure.name, "GapProcedureComplete");
        assert_eq!(gap_procedure.payload, WireEnvelope::bounded(3, 253));

        let bond_lost = coverage.event_metadata.get(&0x0405).unwrap();
        assert_eq!(bond_lost.payload, WireEnvelope::fixed(0));

        let read_multiple = coverage.event_metadata.get(&0x0C15).unwrap();
        assert_eq!(read_multiple.payload, WireEnvelope::bounded(3, 253));
    }

    #[test]
    fn production_command_catalog_uses_the_proc_macro_entry_point() {
        fn inspect(items: &[Item], path: &Path, command_count: &mut usize) {
            for item in items {
                match item {
                    Item::Macro(item) if is_macro_named(&item.mac.path, "vendor_cmd") => {
                        *command_count += 1;
                        let macro_path = item
                            .mac
                            .path
                            .segments
                            .iter()
                            .map(|segment| segment.ident.to_string())
                            .collect::<Vec<_>>();
                        assert_eq!(
                            macro_path,
                            ["stm32wb_hci_macros", "vendor_cmd"],
                            "{} contains a vendor command that bypasses the shared parser",
                            path.display()
                        );
                    }
                    Item::Mod(module) if module.content.is_some() => {
                        let (_, nested) = module.content.as_ref().expect("checked above");
                        inspect(nested, path, command_count);
                    }
                    _ => {}
                }
            }
        }

        let command_dir =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../stm32wb-hci/src/vendor/command");
        let mut command_count = 0;
        for entry in fs::read_dir(&command_dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|extension| extension == "rs")
                && path.file_name().is_some_and(|name| name != "mod.rs")
            {
                let file = read_rust_file(&path).unwrap();
                inspect(&file.items, &path, &mut command_count);
            }
        }
        assert_eq!(command_count, 147);
    }

    #[test]
    fn production_event_catalog_uses_the_proc_macro_and_shared_schema() {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../stm32wb-hci/src/vendor/event/mod.rs");
        let file = read_rust_file(&path).unwrap();
        let macros = file
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Macro(item) if is_macro_named(&item.mac.path, "vendor_event") => Some(item),
                _ => None,
            })
            .collect::<Vec<_>>();
        let [catalog] = macros.as_slice() else {
            panic!("expected exactly one production vendor-event catalog");
        };
        let macro_path = catalog
            .mac
            .path
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect::<Vec<_>>();
        assert_eq!(macro_path, ["stm32wb_hci_macros", "vendor_event"]);

        let parsed = syn::parse2::<SchemaVendorEvents>(catalog.mac.tokens.clone()).unwrap();
        assert_eq!(parsed.events.len(), 56);
    }

    #[test]
    fn loads_unique_command_ids_for_every_declared_firmware() {
        let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../stm32wb-hci");
        for firmware in FirmwareVersion::declared_in_manifest(&crate_dir).unwrap() {
            load_crate_coverage(&crate_dir, firmware).unwrap();
        }
    }

    #[test]
    fn selects_commands_from_descriptor_cfg() {
        let source = r#"
            vendor_cmd! { Current(cgid = 0x0, cid = 0x03) { Params = (); Completion = CommandStatus; } }
            #[cfg(since_fw_0_17_1)]
            vendor_cmd! {
                Retained(cgid = 0x0, cid = 0x01) {
                    Params = ();
                    Completion = CommandStatus;
                }
            }
        "#;
        let path = PathBuf::from("fixture.rs");
        let unit = SourceUnit {
            path,
            active: true,
            file: syn::parse_file(source).unwrap(),
        };
        let firmware = version(0, 17, 0);
        let descriptors = collect_descriptors(std::slice::from_ref(&unit), firmware).unwrap();
        let active = descriptor_coverage(&descriptors);
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].name, "Current");
        assert!(descriptors.contains_key("Current"));
        assert!(!descriptors.contains_key("Retained"));

        let future = collect_descriptors(std::slice::from_ref(&unit), version(0, 18, 0)).unwrap();
        assert!(future.contains_key("Current"));
        assert!(future.contains_key("Retained"));
    }

    #[test]
    fn discovers_only_active_command_modules() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "stm32wb-compliance-command-modules-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).unwrap();
        let root = directory.join("mod.rs");
        fs::write(
            &root,
            r#"
                pub mod current;
                #[cfg(since_fw_0_17_1)]
                pub mod future;
            "#,
        )
        .unwrap();
        fs::write(directory.join("current.rs"), "").unwrap();
        fs::write(directory.join("future.rs"), "").unwrap();

        let mut sources = Vec::new();
        let mut visited = BTreeSet::new();
        collect_command_sources(
            root.clone(),
            read_rust_file(&root).unwrap(),
            true,
            version(0, 17, 0),
            &mut visited,
            &mut sources,
        )
        .unwrap();
        let names = sources
            .iter()
            .filter_map(|source| source.path.file_name().and_then(|name| name.to_str()))
            .collect::<BTreeSet<_>>();
        assert!(names.contains("current.rs"));
        assert!(!names.contains("future.rs"));

        fs::remove_dir_all(directory).unwrap();
    }
}
