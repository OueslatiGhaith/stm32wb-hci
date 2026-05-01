use crate::spec::{
    CommandDoc, CommandSpec, PackedStructSpec, ParamDoc, ParamSpec, PayloadField, StructFieldSpec,
    ValueDoc, WireType, wire_type_for,
};
use anyhow::{Context, Result, anyhow};
use chumsky::prelude::*;
use std::collections::HashMap;

pub fn parse_group(source_name: &str, source: &str, header: &str) -> Result<Vec<CommandSpec>> {
    let docs = parse_command_docs(header)?;
    let mut commands = Vec::new();

    for function in split_functions(source)? {
        let Some(name) = function
            .name
            .strip_prefix("aci_")
            .map(|_| function.name.clone())
        else {
            continue;
        };

        let doc = docs.get(&name);
        let params = parse_signature_params(&function.signature, doc)?;
        let param_types = params
            .iter()
            .map(|p| (p.name.clone(), p.c_type.clone()))
            .collect::<HashMap<_, _>>();

        let ogf = parse_hex_assignment(&function.body, "ogf")?;
        let ocf = parse_hex_assignment(&function.body, "ocf")?;
        let opcode = match (ogf, ocf) {
            (Some(ogf), Some(ocf)) => Some((ogf << 10) | ocf),
            _ => None,
        };

        commands.push(CommandSpec {
            group: source_name.to_owned(),
            name,
            ogf,
            ocf,
            opcode,
            event: parse_hex_assignment(&function.body, "event")?.map(|v| v as u8),
            return_len: parse_decimal_assignment(&function.body, "rlen")?,
            doc: doc.map(|d| d.command.clone()),
            payload: parse_payload(&function.body, &param_types, doc)?,
            params,
        });
    }

    Ok(commands)
}

pub fn parse_packed_structs(source: &str) -> Result<Vec<PackedStructSpec>> {
    let mut structs = Vec::new();
    let mut cursor = 0;

    while let Some(relative_start) = source[cursor..].find("typedef __PACKED_STRUCT") {
        let start = cursor + relative_start;
        let Some(open_brace) = source[start..].find('{').map(|idx| start + idx) else {
            break;
        };
        let close_brace = find_matching_brace(source, open_brace)
            .with_context(|| format!("failed to find packed struct end at byte {start}"))?;
        let after_brace = &source[close_brace + 1..];
        let Some((name, consumed)) = parse_struct_name(after_brace) else {
            cursor = close_brace + 1;
            continue;
        };

        structs.push(PackedStructSpec {
            name,
            fields: parse_struct_fields(&source[open_brace + 1..close_brace])?,
        });

        cursor = close_brace + 1 + consumed;
    }

    Ok(structs)
}

struct Function {
    name: String,
    signature: String,
    body: String,
}

fn split_functions(source: &str) -> Result<Vec<Function>> {
    let mut functions = Vec::new();
    let mut cursor = 0;

    while let Some(relative_start) = source[cursor..].find("tBleStatus") {
        let start = cursor + relative_start;
        let Some(open_brace) = source[start..].find('{').map(|idx| start + idx) else {
            break;
        };
        let Some((name, signature)) = parse_function_header(&source[start..open_brace]) else {
            cursor = open_brace + 1;
            continue;
        };

        let body_start = open_brace + 1;
        let body_end = find_matching_brace(source, body_start - 1)
            .with_context(|| format!("failed to find end of {name}"))?;

        functions.push(Function {
            name,
            signature,
            body: source[body_start..body_end].to_owned(),
        });

        cursor = body_end + 1;
    }

    Ok(functions)
}

fn parse_function_header(input: &str) -> Option<(String, String)> {
    just("tBleStatus")
        .padded()
        .ignore_then(identifier().padded())
        .then(
            none_of(')')
                .repeated()
                .collect::<String>()
                .delimited_by(just('('), just(')'))
                .padded(),
        )
        .then_ignore(end())
        .parse(input)
        .ok()
}

fn find_matching_brace(source: &str, open_brace: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (idx, byte) in source.bytes().enumerate().skip(open_brace) {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(idx);
                }
            }
            _ => {}
        }
    }
    None
}

fn parse_struct_name(input: &str) -> Option<(String, usize)> {
    let trimmed = input.trim_start();
    let skipped = input.len() - trimmed.len();
    let name = identifier().parse(trimmed).ok()?;
    let suffix = &trimmed[name.len()..];
    let rest = suffix.trim_start();
    let after_semicolon = rest.strip_prefix(';')?;
    let consumed = skipped + name.len() + (suffix.len() - after_semicolon.len());
    Some((name, consumed))
}

fn parse_struct_fields(body: &str) -> Result<Vec<StructFieldSpec>> {
    let mut fields = Vec::new();
    let mut pending_doc = None;
    let mut lines = body.lines().peekable();

    while let Some(line) = lines.next() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed.starts_with("/**") {
            let mut doc_lines = Vec::new();
            if trimmed.ends_with("*/") {
                doc_lines.push(trimmed.to_owned());
            } else {
                doc_lines.push(trimmed.to_owned());
                for doc_line in lines.by_ref() {
                    let doc_trimmed = doc_line.trim();
                    doc_lines.push(doc_trimmed.to_owned());
                    if doc_trimmed.ends_with("*/") {
                        break;
                    }
                }
            }
            pending_doc = Some(parse_field_doc(&doc_lines));
            continue;
        }

        if let Some((c_type, name, array_len)) = parse_struct_field_line(trimmed) {
            fields.push(StructFieldSpec {
                wire: wire_type_for(Some(&c_type)),
                c_type,
                name,
                array_len,
                doc: pending_doc.take(),
            });
        }
    }

    Ok(fields)
}

fn parse_field_doc(lines: &[String]) -> ParamDoc {
    let cleaned = lines
        .iter()
        .map(|line| {
            line.trim()
                .trim_start_matches("/**")
                .trim_end_matches("*/")
                .to_owned()
        })
        .map(|line| clean_doc_line(&line))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();

    parse_param_doc(&cleaned)
}

fn parse_struct_field_line(input: &str) -> Option<(String, String, Option<String>)> {
    just("const")
        .padded()
        .or_not()
        .then(identifier().padded())
        .then(just('*').padded().or_not())
        .then(identifier().padded())
        .then(
            none_of(']')
                .repeated()
                .collect::<String>()
                .map(|s| s.trim().to_owned())
                .delimited_by(just('['), just(']'))
                .or_not(),
        )
        .then_ignore(just(';').padded())
        .then_ignore(end())
        .map(|((((is_const, base), pointer), name), array_len)| {
            let mut c_type = String::new();
            if is_const.is_some() {
                c_type.push_str("const ");
            }
            c_type.push_str(&base);
            if pointer.is_some() {
                c_type.push('*');
            }
            (c_type, name, array_len)
        })
        .parse(input)
        .ok()
}

fn parse_signature_params(signature: &str, doc: Option<&CommandDocs>) -> Result<Vec<ParamSpec>> {
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

fn parse_payload(
    body: &str,
    param_types: &HashMap<String, String>,
    doc: Option<&CommandDocs>,
) -> Result<Vec<PayloadField>> {
    let mut payload = Vec::new();

    for line in body.lines().map(str::trim) {
        if let Some(field) = parse_cp_assignment(line) {
            let c_type = param_types.get(&field).cloned();
            payload.push(PayloadField {
                wire: wire_type_for(c_type.as_deref()),
                doc: doc.and_then(|d| d.params.get(&field)).cloned(),
                name: field,
                c_type,
                len: None,
            });
            continue;
        }

        if let Some((field, src, len)) = parse_memcpy(line) {
            payload.push(PayloadField {
                name: field.clone(),
                c_type: param_types.get(&src).cloned(),
                wire: WireType::Bytes,
                len: Some(len),
                doc: doc
                    .and_then(|d| d.params.get(&field).or_else(|| d.params.get(&src)))
                    .cloned(),
            });
        }
    }

    Ok(payload)
}

fn parse_cp_assignment(input: &str) -> Option<String> {
    just("cp")
        .ignore_then(decimal_digits())
        .ignore_then(just("->"))
        .ignore_then(identifier())
        .then_ignore(just('=').padded())
        .then_ignore(any().repeated())
        .then_ignore(end())
        .parse(input)
        .ok()
}

fn parse_memcpy(input: &str) -> Option<(String, String, String)> {
    just("Osal_MemCpy")
        .padded()
        .ignore_then(just('(').padded())
        .ignore_then(just("(void*)").padded())
        .ignore_then(just('&').padded())
        .ignore_then(just("cp"))
        .ignore_then(decimal_digits())
        .ignore_then(just("->"))
        .ignore_then(identifier())
        .then_ignore(just(',').padded())
        .then_ignore(just("(const void*)").padded())
        .then(identifier())
        .then_ignore(just(',').padded())
        .then(
            none_of(')')
                .repeated()
                .collect::<String>()
                .map(|s| s.trim().to_owned()),
        )
        .then_ignore(just(')'))
        .then_ignore(any().repeated())
        .then_ignore(end())
        .map(|((field, src), len)| (field, src, len))
        .parse(input)
        .ok()
}

fn parse_hex_assignment(body: &str, field: &str) -> Result<Option<u16>> {
    parse_rq_assignment(body, field, IntegerBase::Hex)
        .map(|value| Ok(value? as u16))
        .transpose()
}

fn parse_decimal_assignment(body: &str, field: &str) -> Result<Option<usize>> {
    parse_rq_assignment(body, field, IntegerBase::Decimal)
        .map(|value| Ok(value? as usize))
        .transpose()
}

#[derive(Clone, Copy)]
enum IntegerBase {
    Hex,
    Decimal,
}

fn parse_rq_assignment(body: &str, field: &str, base: IntegerBase) -> Option<Result<u64>> {
    body.lines().map(str::trim).find_map(|line| {
        let parsed = match base {
            IntegerBase::Hex => parse_rq_hex_line(line),
            IntegerBase::Decimal => parse_rq_decimal_line(line),
        }?;

        (parsed.0 == field).then_some(Ok(parsed.1))
    })
}

fn parse_rq_hex_line(input: &str) -> Option<(String, u64)> {
    rq_assignment_prefix()
        .then(hex_literal())
        .then_ignore(any().repeated())
        .then_ignore(end())
        .parse(input)
        .ok()
}

fn parse_rq_decimal_line(input: &str) -> Option<(String, u64)> {
    rq_assignment_prefix()
        .then(decimal_digits().try_map(|digits, span| {
            digits
                .parse::<u64>()
                .map_err(|err| Simple::custom(span, err.to_string()))
        }))
        .then_ignore(any().repeated())
        .then_ignore(end())
        .parse(input)
        .ok()
}

fn rq_assignment_prefix() -> impl Parser<char, String, Error = Simple<char>> {
    just("rq.")
        .ignore_then(identifier())
        .then_ignore(just('=').padded())
}

struct CommandDocs {
    command: CommandDoc,
    params: HashMap<String, ParamDoc>,
}

fn parse_command_docs(header: &str) -> Result<HashMap<String, CommandDocs>> {
    let mut docs = HashMap::new();
    let mut cursor = 0;

    while let Some(relative_start) = header[cursor..].find("tBleStatus") {
        let function_start = cursor + relative_start;
        let Some(open_paren) = header[function_start..]
            .find('(')
            .map(|idx| function_start + idx)
        else {
            break;
        };
        let Some(name) = parse_prototype_name(&header[function_start..open_paren]) else {
            cursor = open_paren + 1;
            continue;
        };
        let Some((doc_start, doc_end)) = previous_doc_block(header, function_start) else {
            cursor = open_paren + 1;
            continue;
        };

        docs.insert(name, parse_doc_block(&header[doc_start + 3..doc_end])?);
        cursor = open_paren + 1;
    }

    Ok(docs)
}

fn parse_prototype_name(input: &str) -> Option<String> {
    just("tBleStatus")
        .padded()
        .ignore_then(identifier().padded())
        .then_ignore(end())
        .parse(input)
        .ok()
}

fn previous_doc_block(source: &str, before: usize) -> Option<(usize, usize)> {
    let prefix = &source[..before];
    let doc_end = prefix.rfind("*/")?;
    if !source[doc_end + 2..before].trim().is_empty() {
        return None;
    }

    let doc_start = source[..doc_end].rfind("/**")?;
    Some((doc_start, doc_end))
}

fn parse_doc_block(raw: &str) -> Result<CommandDocs> {
    let lines = raw
        .lines()
        .map(clean_doc_line)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();

    let brief = lines
        .iter()
        .find_map(|line| line.strip_prefix("@brief ").map(str::to_owned));

    let mut description = Vec::new();
    let mut params = HashMap::new();
    let mut current_param: Option<(String, Vec<String>)> = None;

    for line in lines {
        if let Some((name, text)) = parse_param_doc_start(&line) {
            if let Some((name, body)) = current_param.take() {
                params.insert(name, parse_param_doc(&body));
            }
            current_param = Some((name, vec![text]));
        } else if line.starts_with("@return") {
            if let Some((name, body)) = current_param.take() {
                params.insert(name, parse_param_doc(&body));
            }
        } else if let Some((_, body)) = current_param.as_mut() {
            body.push(line);
        } else if !line.starts_with("@brief ") {
            description.push(line);
        }
    }

    if let Some((name, body)) = current_param.take() {
        params.insert(name, parse_param_doc(&body));
    }

    Ok(CommandDocs {
        command: CommandDoc {
            brief,
            description: description.join(" "),
        },
        params,
    })
}

fn parse_param_doc_start(input: &str) -> Option<(String, String)> {
    let bracket = none_of(']').repeated().delimited_by(just('['), just(']'));

    just("@param")
        .ignore_then(bracket.or_not())
        .ignore_then(whitespace1())
        .ignore_then(identifier())
        .then(
            any()
                .repeated()
                .collect::<String>()
                .map(|s| s.trim().to_owned()),
        )
        .then_ignore(end())
        .parse(input)
        .ok()
}

fn parse_param_doc(lines: &[String]) -> ParamDoc {
    ParamDoc {
        description: lines.join(" "),
        values: parse_values(lines),
    }
}

fn parse_values(lines: &[String]) -> Vec<ValueDoc> {
    lines
        .iter()
        .filter_map(|line| {
            let (value, text) = parse_value_line(line.trim())?;
            let raw = line.trim().to_owned();
            let (label, description) = split_label_description(text.as_deref());
            Some(ValueDoc {
                value,
                raw,
                label,
                description,
            })
        })
        .collect()
}

fn parse_value_line(input: &str) -> Option<(u64, Option<String>)> {
    just('-')
        .padded()
        .ignore_then(hex_literal())
        .then(
            just(':')
                .padded()
                .ignore_then(any().repeated().collect::<String>())
                .or_not(),
        )
        .then_ignore(end())
        .map(|(value, text)| {
            (
                value,
                text.map(|text| text.trim().to_owned())
                    .filter(|text| !text.is_empty()),
            )
        })
        .parse(input)
        .ok()
}

fn split_label_description(text: Option<&str>) -> (Option<String>, Option<String>) {
    let Some(text) = text else {
        return (None, None);
    };

    if let Some((label, rest)) = text.split_once('(') {
        return (
            Some(label.trim().to_owned()),
            Some(rest.trim_end_matches(')').trim().to_owned()),
        );
    }

    (Some(text.to_owned()), None)
}

fn clean_doc_line(line: &str) -> String {
    line.trim()
        .trim_start_matches('*')
        .trim()
        .trim_end_matches("<br>")
        .trim()
        .to_owned()
}

fn identifier() -> impl Parser<char, String, Error = Simple<char>> {
    filter(|c: &char| c.is_ascii_alphabetic() || *c == '_')
        .then(filter(|c: &char| c.is_ascii_alphanumeric() || *c == '_').repeated())
        .map(|(first, rest)| {
            let mut ident = String::with_capacity(rest.len() + 1);
            ident.push(first);
            ident.extend(rest);
            ident
        })
}

fn decimal_digits() -> impl Parser<char, String, Error = Simple<char>> {
    filter(|c: &char| c.is_ascii_digit())
        .repeated()
        .at_least(1)
        .collect()
}

fn hex_digits() -> impl Parser<char, String, Error = Simple<char>> {
    filter(|c: &char| c.is_ascii_hexdigit())
        .repeated()
        .at_least(1)
        .collect()
}

fn hex_literal() -> impl Parser<char, u64, Error = Simple<char>> {
    just("0x")
        .ignore_then(hex_digits())
        .try_map(|digits, span| {
            u64::from_str_radix(&digits, 16).map_err(|err| Simple::custom(span, err.to_string()))
        })
}

fn whitespace1() -> impl Parser<char, (), Error = Simple<char>> {
    filter(|c: &char| c.is_whitespace())
        .repeated()
        .at_least(1)
        .ignored()
}

fn format_errors(errors: Vec<Simple<char>>) -> String {
    errors
        .into_iter()
        .map(|error| error.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}
