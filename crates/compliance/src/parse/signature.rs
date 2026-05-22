//! Parser for C function signatures.
//!
//! ST generated command signatures use a small C parameter subset:
//! optional `const`, a base type, optional `*`, then the parameter name.

use super::common::{format_errors, identifier};
use super::docs::CommandDocs;
use crate::spec::ParamSpec;
use anyhow::{Context, Result, anyhow};
use chumsky::prelude::*;

/// Parses command parameters and attaches documentation by parameter name.
pub(super) fn parse_signature_params(
    signature: &str,
    doc: Option<&CommandDocs>,
) -> Result<Vec<ParamSpec>> {
    if signature.trim() == "void" {
        return Ok(Vec::new());
    }

    signature
        .split(',')
        .filter(|p| !p.trim().is_empty())
        .map(|param| {
            let (c_type, name) = parse_c_param(param)
                .with_context(|| format!("unsupported param syntax: {param:?}"))?;
            Ok(ParamSpec {
                c_type,
                doc: doc.and_then(|d| d.params.get(&name)).cloned(),
                name,
            })
        })
        .collect()
}

/// Parses one C parameter into `(c_type, name)`.
fn parse_c_param(input: &str) -> Result<(String, String)> {
    just("const")
        .padded()
        .or_not()
        .then(identifier().padded())
        .then(just('*').padded().or_not())
        .then(identifier().padded())
        .then_ignore(end())
        .map(|(((is_const, base), pointer), name)| {
            let mut c_type = String::new();
            if is_const.is_some() {
                c_type.push_str("const ");
            }
            c_type.push_str(&base);
            if pointer.is_some() {
                c_type.push('*');
            }
            (c_type, name)
        })
        .parse(input)
        .map_err(|errors| anyhow!("failed to parse C param: {}", format_errors(errors)))
}
