//! A deliberately small, source-local C preprocessor for generated CubeWB C.
//!
//! The compliance checker reads a tagged file in isolation rather than
//! compiling the CubeWB middleware.  Tree-sitter represents both sides of a
//! preprocessor branch, so passing that raw tree to an extractor would turn an
//! inactive declaration into a false protocol claim.  This module evaluates
//! the branch directives and masks inactive bytes before parsing.  It follows
//! C preprocessor rules that matter to generated API files: object-like
//! `#define`s, `#undef`, `defined`, integer expressions, and nested
//! `#if`/`#elif`/`#else` blocks.  It intentionally does not follow `#include`s
//! or expand function-like macros; the tagged source itself remains the sole
//! authority for this source-local view.

use std::collections::BTreeMap;

#[derive(Clone, Debug)]
enum MacroDefinition {
    Object(String),
    Function,
}

#[derive(Clone, Copy, Debug)]
struct ConditionalGroup {
    parent_active: bool,
    branch_taken: bool,
    current_active: bool,
    saw_else: bool,
}

#[derive(Clone, Copy, Debug, Default)]
struct LexicalState {
    block_comment: bool,
    quote: Option<u8>,
}

#[derive(Clone, Debug)]
enum Directive {
    If(String),
    Ifdef(String),
    Ifndef(String),
    Elif(String),
    Elifdef(String),
    Elifndef(String),
    Else,
    Endif,
    Define {
        name: String,
        definition: MacroDefinition,
    },
    Undef(String),
    Other,
}

/// Return `source` with inactive branches and all preprocessing directives
/// replaced by spaces. Newlines and byte offsets are preserved exactly, so
/// Tree-sitter nodes can still be read from the original source text.
pub(crate) fn preprocess_c_source(source: &str, source_name: &str) -> Result<String, String> {
    let lines = line_ranges(source);
    let directive_starts = directive_starts(source, &lines);
    let mut output = source.as_bytes().to_vec();
    let mut macros = BTreeMap::<String, MacroDefinition>::new();
    let mut groups = Vec::<ConditionalGroup>::new();
    let mut index = 0;

    while index < lines.len() {
        if directive_starts[index] {
            let (directive, next) = parse_directive(source, &lines, index)
                .map_err(|message| format!("{source_name}: {message}"))?;
            for &(start, end) in &lines[index..next] {
                mask_range(&mut output, start, end);
            }
            apply_directive(directive, &mut macros, &mut groups)
                .map_err(|message| format!("{source_name}: {message}"))?;
            index = next;
        } else {
            if !groups.last().is_none_or(|group| group.current_active) {
                let (start, end) = lines[index];
                mask_range(&mut output, start, end);
            }
            index += 1;
        }
    }

    if !groups.is_empty() {
        return Err(format!(
            "{source_name}: unterminated C preprocessor conditional"
        ));
    }
    Ok(String::from_utf8(output).expect("masking ASCII bytes preserves valid UTF-8"))
}

fn directive_starts(source: &str, lines: &[(usize, usize)]) -> Vec<bool> {
    let mut state = LexicalState::default();
    lines
        .iter()
        .map(|&(start, end)| line_starts_directive(&source[start..end], &mut state))
        .collect()
}

/// Determine whether a physical source line begins with a preprocessing
/// directive. This avoids treating `#if` examples in comments or strings as
/// real directives while still allowing comments before a directive, as C
/// preprocessing does.
fn line_starts_directive(line: &str, state: &mut LexicalState) -> bool {
    let bytes = line.as_bytes();
    let mut index = 0;
    let mut directive_position = state.quote.is_none();
    let mut directive = false;

    while index < bytes.len() {
        if state.block_comment {
            if bytes[index] == b'*' && bytes.get(index + 1) == Some(&b'/') {
                state.block_comment = false;
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }

        if let Some(quote) = state.quote {
            if bytes[index] == b'\\' {
                index += 2;
            } else {
                if bytes[index] == quote {
                    state.quote = None;
                }
                index += 1;
            }
            continue;
        }

        if bytes[index] == b'/' && bytes.get(index + 1) == Some(&b'*') {
            state.block_comment = true;
            index += 2;
            continue;
        }
        if bytes[index] == b'/' && bytes.get(index + 1) == Some(&b'/') {
            break;
        }
        if matches!(bytes[index], b'\'' | b'\"') {
            state.quote = Some(bytes[index]);
            directive_position = false;
            index += 1;
            continue;
        }
        if directive_position && bytes[index].is_ascii_whitespace() {
            index += 1;
            continue;
        }
        if directive_position && bytes[index] == b'#' {
            directive = true;
        }
        directive_position = false;
        index += 1;
    }
    directive
}

fn line_ranges(source: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut start = 0;
    for (index, byte) in source.bytes().enumerate() {
        if byte == b'\n' {
            ranges.push((start, index + 1));
            start = index + 1;
        }
    }
    if start < source.len() {
        ranges.push((start, source.len()));
    }
    ranges
}

fn parse_directive(
    source: &str,
    lines: &[(usize, usize)],
    start: usize,
) -> Result<(Directive, usize), String> {
    let mut text = String::new();
    let mut index = start;
    loop {
        let (line_start, line_end) = lines[index];
        let line = source[line_start..line_end].trim_end_matches(['\r', '\n']);
        if let Some(without_continuation) = line.strip_suffix('\\') {
            text.push_str(without_continuation);
            text.push(' ');
            index += 1;
            if index == lines.len() {
                return Err("preprocessor directive ends with a line continuation".to_owned());
            }
        } else {
            text.push_str(line);
            break;
        }
    }

    let payload = text
        .trim_start()
        .strip_prefix('#')
        .expect("a directive was detected above")
        .trim_start();
    let (keyword, remainder) = split_word(payload);
    let directive = match keyword {
        "if" => Directive::If(required_expression(remainder, "#if")?),
        "ifdef" => Directive::Ifdef(required_identifier(remainder, "#ifdef")?),
        "ifndef" => Directive::Ifndef(required_identifier(remainder, "#ifndef")?),
        "elif" => Directive::Elif(required_expression(remainder, "#elif")?),
        "elifdef" => Directive::Elifdef(required_identifier(remainder, "#elifdef")?),
        "elifndef" => Directive::Elifndef(required_identifier(remainder, "#elifndef")?),
        "else" => Directive::Else,
        "endif" => Directive::Endif,
        "define" => parse_define(remainder)?,
        "undef" => Directive::Undef(required_identifier(remainder, "#undef")?),
        _ => Directive::Other,
    };
    Ok((directive, index + 1))
}

fn split_word(value: &str) -> (&str, &str) {
    let end = value
        .bytes()
        .take_while(|byte| byte.is_ascii_alphabetic())
        .count();
    (&value[..end], value[end..].trim_start())
}

fn required_expression(value: &str, directive: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() {
        Err(format!("{directive} requires an expression"))
    } else {
        Ok(value.to_owned())
    }
}

fn required_identifier(value: &str, directive: &str) -> Result<String, String> {
    let (identifier, _) = split_identifier(value.trim_start());
    if identifier.is_empty() {
        Err(format!("{directive} requires an identifier"))
    } else {
        Ok(identifier.to_owned())
    }
}

fn parse_define(value: &str) -> Result<Directive, String> {
    let value = value.trim_start();
    let (name, remainder) = split_identifier(value);
    if name.is_empty() {
        return Err("#define requires an identifier".to_owned());
    }
    let definition = if remainder.starts_with('(') {
        MacroDefinition::Function
    } else {
        let replacement = remainder.trim();
        let replacement = if replacement.is_empty() {
            "1"
        } else {
            replacement
        };
        MacroDefinition::Object(replacement.to_owned())
    };
    Ok(Directive::Define {
        name: name.to_owned(),
        definition,
    })
}

fn split_identifier(value: &str) -> (&str, &str) {
    let end = value
        .bytes()
        .take_while(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
        .count();
    (&value[..end], &value[end..])
}

fn mask_range(output: &mut [u8], start: usize, end: usize) {
    for byte in &mut output[start..end] {
        if !matches!(*byte, b'\r' | b'\n') {
            *byte = b' ';
        }
    }
}

fn apply_directive(
    directive: Directive,
    macros: &mut BTreeMap<String, MacroDefinition>,
    groups: &mut Vec<ConditionalGroup>,
) -> Result<(), String> {
    let active = groups.last().is_none_or(|group| group.current_active);
    match directive {
        Directive::If(expression) => push_group(groups, active, || evaluate(&expression, macros)),
        Directive::Ifdef(name) => push_group(groups, active, || Ok(macros.contains_key(&name))),
        Directive::Ifndef(name) => push_group(groups, active, || Ok(!macros.contains_key(&name))),
        Directive::Elif(expression) => select_elif(groups, || evaluate(&expression, macros)),
        Directive::Elifdef(name) => select_elif(groups, || Ok(macros.contains_key(&name))),
        Directive::Elifndef(name) => select_elif(groups, || Ok(!macros.contains_key(&name))),
        Directive::Else => select_else(groups),
        Directive::Endif => groups
            .pop()
            .map(|_| ())
            .ok_or_else(|| "#endif has no matching #if".to_owned()),
        Directive::Define { name, definition } if active => {
            macros.insert(name, definition);
            Ok(())
        }
        Directive::Undef(name) if active => {
            macros.remove(&name);
            Ok(())
        }
        Directive::Define { .. } | Directive::Undef(_) | Directive::Other => Ok(()),
    }
}

fn push_group(
    groups: &mut Vec<ConditionalGroup>,
    parent_active: bool,
    condition: impl FnOnce() -> Result<bool, String>,
) -> Result<(), String> {
    let selected = parent_active && condition()?;
    groups.push(ConditionalGroup {
        parent_active,
        branch_taken: selected,
        current_active: selected,
        saw_else: false,
    });
    Ok(())
}

fn select_elif(
    groups: &mut [ConditionalGroup],
    condition: impl FnOnce() -> Result<bool, String>,
) -> Result<(), String> {
    let group = groups
        .last_mut()
        .ok_or_else(|| "#elif has no matching #if".to_owned())?;
    if group.saw_else {
        return Err("#elif appears after #else".to_owned());
    }
    let selected = group.parent_active && !group.branch_taken && condition()?;
    group.current_active = selected;
    group.branch_taken |= selected;
    Ok(())
}

fn select_else(groups: &mut [ConditionalGroup]) -> Result<(), String> {
    let group = groups
        .last_mut()
        .ok_or_else(|| "#else has no matching #if".to_owned())?;
    if group.saw_else {
        return Err("#if has more than one #else".to_owned());
    }
    group.current_active = group.parent_active && !group.branch_taken;
    group.branch_taken = true;
    group.saw_else = true;
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Operator {
    LogicalOr,
    LogicalAnd,
    BitOr,
    BitXor,
    BitAnd,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    ShiftLeft,
    ShiftRight,
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    Not,
    BitNot,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Token {
    Number(i64),
    Identifier(String),
    LeftParen,
    RightParen,
    Operator(Operator),
}

fn evaluate(expression: &str, macros: &BTreeMap<String, MacroDefinition>) -> Result<bool, String> {
    let tokens = tokenize(expression)?;
    let mut expansions = Vec::new();
    let value = ExpressionParser::new(tokens, macros, &mut expansions).parse()?;
    Ok(value != 0)
}

fn tokenize(expression: &str) -> Result<Vec<Token>, String> {
    let bytes = expression.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            byte if byte.is_ascii_whitespace() => index += 1,
            b'/' if bytes.get(index + 1) == Some(&b'/') => break,
            b'/' if bytes.get(index + 1) == Some(&b'*') => {
                let Some(end) = expression[index + 2..].find("*/") else {
                    return Err("unterminated comment in #if expression".to_owned());
                };
                index += end + 4;
            }
            byte if byte.is_ascii_alphabetic() || byte == b'_' => {
                let start = index;
                index += 1;
                while bytes
                    .get(index)
                    .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
                {
                    index += 1;
                }
                tokens.push(Token::Identifier(expression[start..index].to_owned()));
            }
            byte if byte.is_ascii_digit() => {
                let (number, end) = parse_number(expression, index)?;
                tokens.push(Token::Number(number));
                index = end;
            }
            b'(' => {
                tokens.push(Token::LeftParen);
                index += 1;
            }
            b')' => {
                tokens.push(Token::RightParen);
                index += 1;
            }
            _ => {
                let (operator, width) = match &bytes[index..] {
                    [b'|', b'|', ..] => (Operator::LogicalOr, 2),
                    [b'&', b'&', ..] => (Operator::LogicalAnd, 2),
                    [b'=', b'=', ..] => (Operator::Equal, 2),
                    [b'!', b'=', ..] => (Operator::NotEqual, 2),
                    [b'<', b'=', ..] => (Operator::LessEqual, 2),
                    [b'>', b'=', ..] => (Operator::GreaterEqual, 2),
                    [b'<', b'<', ..] => (Operator::ShiftLeft, 2),
                    [b'>', b'>', ..] => (Operator::ShiftRight, 2),
                    [b'|', ..] => (Operator::BitOr, 1),
                    [b'^', ..] => (Operator::BitXor, 1),
                    [b'&', ..] => (Operator::BitAnd, 1),
                    [b'<', ..] => (Operator::Less, 1),
                    [b'>', ..] => (Operator::Greater, 1),
                    [b'+', ..] => (Operator::Add, 1),
                    [b'-', ..] => (Operator::Subtract, 1),
                    [b'*', ..] => (Operator::Multiply, 1),
                    [b'/', ..] => (Operator::Divide, 1),
                    [b'%', ..] => (Operator::Remainder, 1),
                    [b'!', ..] => (Operator::Not, 1),
                    [b'~', ..] => (Operator::BitNot, 1),
                    _ => {
                        return Err(format!(
                            "unsupported token `{}` in #if expression",
                            expression[index..].chars().next().unwrap_or_default()
                        ));
                    }
                };
                tokens.push(Token::Operator(operator));
                index += width;
            }
        }
    }
    Ok(tokens)
}

fn parse_number(expression: &str, start: usize) -> Result<(i64, usize), String> {
    let bytes = expression.as_bytes();
    let mut end = start;
    let hexadecimal =
        bytes.get(start) == Some(&b'0') && matches!(bytes.get(start + 1), Some(b'x' | b'X'));
    if hexadecimal {
        end += 2;
        let digits = end;
        while bytes.get(end).is_some_and(u8::is_ascii_hexdigit) {
            end += 1;
        }
        if end == digits {
            return Err("hexadecimal integer has no digits in #if expression".to_owned());
        }
    } else {
        while bytes.get(end).is_some_and(u8::is_ascii_digit) {
            end += 1;
        }
    }
    let digits_end = end;
    while bytes
        .get(end)
        .is_some_and(|byte| matches!(byte, b'u' | b'U' | b'l' | b'L'))
    {
        end += 1;
    }
    let literal = &expression[start..digits_end];
    let (radix, digits) = if hexadecimal {
        (16, &literal[2..])
    } else if literal.len() > 1 && literal.starts_with('0') {
        (8, literal)
    } else {
        (10, literal)
    };
    let number = i64::from_str_radix(digits, radix)
        .map_err(|_| format!("invalid integer literal `{literal}` in #if expression"))?;
    Ok((number, end))
}

struct ExpressionParser<'a> {
    tokens: Vec<Token>,
    position: usize,
    macros: &'a BTreeMap<String, MacroDefinition>,
    expansions: &'a mut Vec<String>,
}

impl<'a> ExpressionParser<'a> {
    fn new(
        tokens: Vec<Token>,
        macros: &'a BTreeMap<String, MacroDefinition>,
        expansions: &'a mut Vec<String>,
    ) -> Self {
        Self {
            tokens,
            position: 0,
            macros,
            expansions,
        }
    }

    fn parse(mut self) -> Result<i64, String> {
        let value = self.logical_or()?;
        if let Some(token) = self.tokens.get(self.position) {
            return Err(format!("unexpected {token:?} in #if expression"));
        }
        Ok(value)
    }

    fn logical_or(&mut self) -> Result<i64, String> {
        self.binary(
            Self::logical_and,
            &[Operator::LogicalOr],
            |_, left, right| Ok(i64::from(left != 0 || right != 0)),
        )
    }

    fn logical_and(&mut self) -> Result<i64, String> {
        self.binary(Self::bit_or, &[Operator::LogicalAnd], |_, left, right| {
            Ok(i64::from(left != 0 && right != 0))
        })
    }

    fn bit_or(&mut self) -> Result<i64, String> {
        self.binary(Self::bit_xor, &[Operator::BitOr], |_, left, right| {
            Ok(left | right)
        })
    }

    fn bit_xor(&mut self) -> Result<i64, String> {
        self.binary(Self::bit_and, &[Operator::BitXor], |_, left, right| {
            Ok(left ^ right)
        })
    }

    fn bit_and(&mut self) -> Result<i64, String> {
        self.binary(Self::equality, &[Operator::BitAnd], |_, left, right| {
            Ok(left & right)
        })
    }

    fn equality(&mut self) -> Result<i64, String> {
        self.binary(
            Self::comparison,
            &[Operator::Equal, Operator::NotEqual],
            |operator, left, right| {
                Ok(i64::from(match operator {
                    Operator::Equal => left == right,
                    Operator::NotEqual => left != right,
                    _ => unreachable!("equality operators are selected above"),
                }))
            },
        )
    }

    fn comparison(&mut self) -> Result<i64, String> {
        self.binary(
            Self::shift,
            &[
                Operator::Less,
                Operator::LessEqual,
                Operator::Greater,
                Operator::GreaterEqual,
            ],
            |operator, left, right| {
                Ok(i64::from(match operator {
                    Operator::Less => left < right,
                    Operator::LessEqual => left <= right,
                    Operator::Greater => left > right,
                    Operator::GreaterEqual => left >= right,
                    _ => unreachable!("comparison operators are selected above"),
                }))
            },
        )
    }

    fn shift(&mut self) -> Result<i64, String> {
        self.binary(
            Self::additive,
            &[Operator::ShiftLeft, Operator::ShiftRight],
            |operator, left, right| {
                let shift = u32::try_from(right)
                    .ok()
                    .filter(|shift| *shift < i64::BITS)
                    .ok_or_else(|| "invalid shift in #if expression".to_owned())?;
                match operator {
                    Operator::ShiftLeft => left
                        .checked_shl(shift)
                        .ok_or_else(|| "integer overflow in #if expression".to_owned()),
                    Operator::ShiftRight => Ok(left >> shift),
                    _ => unreachable!("shift operators are selected above"),
                }
            },
        )
    }

    fn additive(&mut self) -> Result<i64, String> {
        self.binary(
            Self::multiplicative,
            &[Operator::Add, Operator::Subtract],
            |operator, left, right| match operator {
                Operator::Add => left
                    .checked_add(right)
                    .ok_or_else(|| "integer overflow in #if expression".to_owned()),
                Operator::Subtract => left
                    .checked_sub(right)
                    .ok_or_else(|| "integer overflow in #if expression".to_owned()),
                _ => unreachable!("additive operators are selected above"),
            },
        )
    }

    fn multiplicative(&mut self) -> Result<i64, String> {
        self.binary(
            Self::unary,
            &[Operator::Multiply, Operator::Divide, Operator::Remainder],
            |operator, left, right| match operator {
                Operator::Multiply => left
                    .checked_mul(right)
                    .ok_or_else(|| "integer overflow in #if expression".to_owned()),
                Operator::Divide => left
                    .checked_div(right)
                    .ok_or_else(|| "invalid division in #if expression".to_owned()),
                Operator::Remainder => left
                    .checked_rem(right)
                    .ok_or_else(|| "invalid remainder in #if expression".to_owned()),
                _ => unreachable!("multiplicative operators are selected above"),
            },
        )
    }

    fn binary(
        &mut self,
        next: fn(&mut Self) -> Result<i64, String>,
        operators: &[Operator],
        combine: impl Fn(Operator, i64, i64) -> Result<i64, String>,
    ) -> Result<i64, String> {
        let mut value = next(self)?;
        while let Some(operator) = self.operator() {
            if !operators.contains(&operator) {
                break;
            }
            self.position += 1;
            let right = next(self)?;
            value = combine(operator, value, right)?;
        }
        Ok(value)
    }

    fn unary(&mut self) -> Result<i64, String> {
        if let Some(
            operator @ (Operator::Not | Operator::BitNot | Operator::Add | Operator::Subtract),
        ) = self.operator()
        {
            self.position += 1;
            let value = self.unary()?;
            return match operator {
                Operator::Not => Ok(i64::from(value == 0)),
                Operator::BitNot => Ok(!value),
                Operator::Add => Ok(value),
                Operator::Subtract => value
                    .checked_neg()
                    .ok_or_else(|| "integer overflow in #if expression".to_owned()),
                _ => unreachable!("unary operators are selected above"),
            };
        }
        self.primary()
    }

    fn primary(&mut self) -> Result<i64, String> {
        let Some(token) = self.tokens.get(self.position).cloned() else {
            return Err("expected an operand in #if expression".to_owned());
        };
        self.position += 1;
        match token {
            Token::Number(value) => Ok(value),
            Token::Identifier(name) if name == "defined" => self.defined(),
            Token::Identifier(name) => self.macro_value(&name),
            Token::LeftParen => {
                let value = self.logical_or()?;
                if !matches!(self.tokens.get(self.position), Some(Token::RightParen)) {
                    return Err("expected `)` in #if expression".to_owned());
                }
                self.position += 1;
                Ok(value)
            }
            token => Err(format!(
                "expected an operand in #if expression, found {token:?}"
            )),
        }
    }

    fn defined(&mut self) -> Result<i64, String> {
        let parenthesized = matches!(self.tokens.get(self.position), Some(Token::LeftParen));
        if parenthesized {
            self.position += 1;
        }
        let Some(Token::Identifier(name)) = self.tokens.get(self.position) else {
            return Err("defined requires an identifier in #if expression".to_owned());
        };
        let defined = self.macros.contains_key(name);
        self.position += 1;
        if parenthesized {
            if !matches!(self.tokens.get(self.position), Some(Token::RightParen)) {
                return Err("defined requires a closing `)` in #if expression".to_owned());
            }
            self.position += 1;
        }
        Ok(i64::from(defined))
    }

    fn macro_value(&mut self, name: &str) -> Result<i64, String> {
        let Some(definition) = self.macros.get(name) else {
            // C replaces an unexpanded identifier in a preprocessor integer
            // expression with zero.
            return Ok(0);
        };
        let MacroDefinition::Object(expression) = definition else {
            // A function-like macro only expands when it is invoked. As a bare
            // identifier in #if it has the same zero value as an undefined one.
            return Ok(0);
        };
        if self.expansions.iter().any(|expanded| expanded == name) {
            return Err(format!("recursive macro `{name}` in #if expression"));
        }
        self.expansions.push(name.to_owned());
        let value = evaluate_expression(expression, self.macros, self.expansions);
        self.expansions.pop();
        value
    }

    fn operator(&self) -> Option<Operator> {
        match self.tokens.get(self.position) {
            Some(Token::Operator(operator)) => Some(*operator),
            _ => None,
        }
    }
}

fn evaluate_expression(
    expression: &str,
    macros: &BTreeMap<String, MacroDefinition>,
    expansions: &mut Vec<String>,
) -> Result<i64, String> {
    ExpressionParser::new(tokenize(expression)?, macros, expansions).parse()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_only_the_selected_nested_branch_and_preserves_offsets() {
        let source = r#"
#define API_LEVEL 2
#if API_LEVEL >= 2 && !defined(DISABLED)
int current;
#if 0
int nested_old;
#else
int nested_current;
#endif
#elif API_LEVEL == 1
int old;
#else
int future;
#endif
"#;
        let filtered = preprocess_c_source(source, "fixture.c").unwrap();
        assert_eq!(filtered.len(), source.len());
        assert!(filtered.contains("int current;"));
        assert!(filtered.contains("int nested_current;"));
        assert!(!filtered.contains("int nested_old;"));
        assert!(!filtered.contains("int old;"));
        assert!(!filtered.contains("int future;"));
    }

    #[test]
    fn inactive_defines_do_not_affect_later_conditions() {
        let source = r#"
#if 0
#define ENABLED 1
#endif
#if defined(ENABLED)
int wrong;
#else
int right;
#endif
"#;
        let filtered = preprocess_c_source(source, "fixture.c").unwrap();
        assert!(filtered.contains("int right;"));
        assert!(!filtered.contains("int wrong;"));
    }

    #[test]
    fn ignores_directive_examples_inside_comments_and_strings() {
        let source = r##"
/*
#if 0
*/
const char *example = "#else";
int active;
"##;
        let filtered = preprocess_c_source(source, "fixture.c").unwrap();
        assert!(filtered.contains("int active;"));
    }

    #[test]
    fn rejects_unbalanced_conditionals() {
        let error = preprocess_c_source("#if 1\nint value;\n", "fixture.c").unwrap_err();
        assert!(error.contains("unterminated"));
    }
}
