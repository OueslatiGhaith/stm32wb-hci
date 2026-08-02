//! Structural evaluation of the Rust cfg subset used by the checked crate.
//!
//! Compliance extraction must fail closed when it encounters a predicate it
//! does not understand. Both the vendor catalog and local standard-HCI
//! extensions use this evaluator so they cannot disagree about which items are
//! active for a Cube release and stack profile.

use std::path::Path;

use syn::punctuated::Punctuated;
use syn::{Attribute, Expr, Lit, Meta, Path as SynPath};

use crate::ComplianceTarget;

/// Evaluate all `#[cfg]` and `#[cfg_attr]` attributes on one syntax node.
pub(crate) fn attrs_active(
    attributes: &[Attribute],
    target: ComplianceTarget,
    path: &Path,
) -> Result<bool, String> {
    let mut active = true;
    for attribute in attributes {
        if attribute.path().is_ident("cfg") {
            active &= eval_cfg_attribute(&attribute.meta, target, path)?;
        } else if attribute.path().is_ident("cfg_attr") {
            active &= eval_cfg_attr_attribute(&attribute.meta, target, path)?;
        }
    }
    Ok(active)
}

fn eval_cfg_attribute(meta: &Meta, target: ComplianceTarget, path: &Path) -> Result<bool, String> {
    let Meta::List(list) = meta else {
        return Err(format!("{}: #[cfg] must use parentheses", path.display()));
    };
    let conditions = parse_meta_list(list, path, "#[cfg(...)] condition")?;
    let conditions = conditions.iter().collect::<Vec<_>>();
    let [condition] = conditions.as_slice() else {
        return Err(format!(
            "{}: #[cfg(...)] requires exactly one condition",
            path.display()
        ));
    };
    eval_cfg_meta(condition, target, path)
}

fn eval_cfg_attr_attribute(
    meta: &Meta,
    target: ComplianceTarget,
    path: &Path,
) -> Result<bool, String> {
    let Meta::List(list) = meta else {
        return Err(format!(
            "{}: #[cfg_attr] must use parentheses",
            path.display()
        ));
    };
    let values = parse_meta_list(list, path, "#[cfg_attr(...)] condition")?;
    let mut values = values.iter();
    let Some(condition) = values.next() else {
        return Err(format!("{}: #[cfg_attr] has no condition", path.display()));
    };
    if !eval_cfg_meta(condition, target, path)? {
        return Ok(true);
    }

    let mut active = true;
    for generated in values {
        active &= eval_generated_cfg_attribute(generated, target, path)?;
    }
    Ok(active)
}

fn eval_generated_cfg_attribute(
    generated: &Meta,
    target: ComplianceTarget,
    path: &Path,
) -> Result<bool, String> {
    if generated.path().is_ident("cfg") {
        return eval_cfg_attribute(generated, target, path);
    }
    if generated.path().is_ident("cfg_attr") {
        return eval_cfg_attr_attribute(generated, target, path);
    }
    Ok(true)
}

fn eval_cfg_meta(meta: &Meta, target: ComplianceTarget, path: &Path) -> Result<bool, String> {
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
            if let Some(value) = target.release.matches_version_cfg(&name) {
                return Ok(value);
            }
            match name.as_str() {
                // Compliance runs `cargo check`, not tests or rustdoc.
                "test" | "doctest" | "doc" => Ok(false),
                // `cargo check` uses the development profile by default.
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
            Ok(feature.value() == target.release.feature_name()
                || feature.value() == target.profile.feature_name())
        }
        Meta::NameValue(value) => Err(format!(
            "{}: unsupported cfg key `{}`",
            path.display(),
            cfg_path_name(&value.path)
        )),
        Meta::List(list) if list.path.is_ident("all") => parse_meta_list(list, path, "cfg list")?
            .iter()
            .map(|value| eval_cfg_meta(value, target, path))
            .try_fold(true, |active, value| value.map(|value| active && value)),
        Meta::List(list) if list.path.is_ident("any") => parse_meta_list(list, path, "cfg list")?
            .iter()
            .map(|value| eval_cfg_meta(value, target, path))
            .try_fold(false, |active, value| value.map(|value| active || value)),
        Meta::List(list) if list.path.is_ident("not") => {
            let values = parse_meta_list(list, path, "cfg list")?;
            let values = values.iter().collect::<Vec<_>>();
            let [value] = values.as_slice() else {
                return Err(format!(
                    "{}: cfg(not(...)) requires exactly one predicate",
                    path.display()
                ));
            };
            Ok(!eval_cfg_meta(value, target, path)?)
        }
        Meta::List(list) => Err(format!(
            "{}: unsupported cfg combinator `{}`",
            path.display(),
            cfg_path_name(&list.path)
        )),
    }
}

fn parse_meta_list(
    list: &syn::MetaList,
    path: &Path,
    description: &str,
) -> Result<Punctuated<Meta, syn::Token![,]>, String> {
    list.parse_args_with(Punctuated::<Meta, syn::Token![,]>::parse_terminated)
        .map_err(|error| format!("{}: could not parse {description}: {error}", path.display()))
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

    fn attributes(source: &str) -> Vec<Attribute> {
        syn::parse_str::<syn::ItemStruct>(source).unwrap().attrs
    }

    fn target(release: crate::FirmwareVersion, profile: crate::StackProfile) -> ComplianceTarget {
        ComplianceTarget::new(release, crate::McuFamily::Wb5x, profile)
    }

    #[test]
    fn evaluates_nested_firmware_cfgs_and_features() {
        let path = Path::new("fixture.rs");
        let old = crate::FirmwareVersion::new(1, 16, 0);
        let current = crate::FirmwareVersion::new(1, 17, 0);
        let attrs = attributes(
            r#"
                #[cfg(all(since_fw_1_17_0, not(since_fw_1_17_1)))]
                struct Fixture;
            "#,
        );
        assert!(!attrs_active(&attrs, target(old, crate::StackProfile::Full), path).unwrap());
        assert!(attrs_active(&attrs, target(current, crate::StackProfile::Full), path).unwrap());

        let attrs = attributes(
            r#"
                #[cfg(feature = "fw_1_17_0")]
                struct Fixture;
            "#,
        );
        assert!(attrs_active(&attrs, target(current, crate::StackProfile::Full), path).unwrap());

        let attrs = attributes(
            r#"
                #[cfg(any(feature = "stack-full-extended", feature = "stack-light"))]
                struct Fixture;
            "#,
        );
        assert!(attrs_active(&attrs, target(current, crate::StackProfile::Light), path).unwrap());
        assert!(
            !attrs_active(&attrs, target(current, crate::StackProfile::HciLayer), path).unwrap()
        );
    }

    #[test]
    fn evaluates_cfg_attr_and_rejects_unknown_predicates() {
        let path = Path::new("fixture.rs");
        let firmware = crate::FirmwareVersion::new(1, 17, 0);
        let attrs = attributes(
            r#"
                #[cfg_attr(since_fw_1_17_0, cfg(not(feature = "fw_1_17_0")))]
                struct Fixture;
            "#,
        );
        assert!(!attrs_active(&attrs, target(firmware, crate::StackProfile::Full), path).unwrap());

        let attrs = attributes("#[cfg(target_os = \"none\")] struct Fixture;");
        assert!(attrs_active(&attrs, target(firmware, crate::StackProfile::Full), path).is_err());
    }
}
