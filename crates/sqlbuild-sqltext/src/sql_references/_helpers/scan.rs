//! The general reference scan of Python `extract_sql_references`, reproduced exactly.

use crate::resolution::main::code_start::code_start;
use crate::resolution::main::over_clause_open::over_clause_open;
use crate::sql_references::constants::{
    DBT_REFERENCE_KIND, NAME_QUOTE_BYTES, NON_CODE_START_BYTES, PARTITION_CLAUSE_WORDS,
    PLACEHOLDER_NAMES, QUOTE_BYTES, QUOTED_NAME_MINIMUM_BYTES, REFERENCE_CONTEXT,
    REFERENCE_PREFIXES, TABLE_FUNCTION_CALL_CONTEXT, TABLE_FUNCTION_REFERENCE_KIND,
};
use crate::sql_references::models::{InvalidReferenceCall, ReferenceScan, SqlReference};
use crate::sql_references::types::ReferencePrefix;
use crate::sql_scan::main::dialect_non_code_end::dialect_non_code_end;
use crate::sql_scan::models::{LexicalSyntax, Unclosed};

/// The `CompileInputError` message that stopped the scan.
type Scan<T> = Result<T, String>;

/// The scan error and the byte offset of the quote or comment, or the call, it points at.
pub(crate) type Stopped = (String, usize);

const NON_CODE_START: [bool; 256] = byte_table(NON_CODE_START_BYTES, false);
const REFERENCE_SCAN_START: [bool; 256] = byte_table(NON_CODE_START_BYTES, true);

/// Mark `bytes`, and the underscore that starts every reference call when `underscore` is set.
const fn byte_table(bytes: &[u8], underscore: bool) -> [bool; 256] {
    let mut table = [false; 256];
    let mut index = 0;
    while index < bytes.len() {
        table[bytes[index] as usize] = true;
        index += 1;
    }
    table[b'_' as usize] = underscore;
    table
}

/// One parsed call: a reference, or a rejected call without its start.
enum Parsed {
    Reference(SqlReference),
    Invalid(InvalidReferenceCall),
}

/// Return every reference and rejected call in `sql` in authored order.
pub(crate) fn scan_references(
    text: &str,
    syntax: &LexicalSyntax,
) -> Result<ReferenceScan, Stopped> {
    let sql: &[u8] = text.as_bytes();
    let mut scan: ReferenceScan = ReferenceScan::default();
    let mut offsets: CharOffsets = CharOffsets::default();
    let mut index = 0;
    while index < sql.len() {
        if let Some(end) =
            non_code_end(sql, index, syntax, REFERENCE_CONTEXT).map_err(|stop| (stop, index))?
        {
            index = end;
            continue;
        }
        if sql[index] != b'_' {
            index += 1;
            while index < sql.len() && !REFERENCE_SCAN_START[usize::from(sql[index])] {
                index += 1;
            }
            continue;
        }
        let Some(call) = REFERENCE_PREFIXES
            .iter()
            .find(|(prefix, _)| sql[index..].starts_with(prefix))
        else {
            index += 1;
            continue;
        };
        let (parsed, next) =
            parse_reference(text, index, call, syntax).map_err(|stop| (stop, index))?;
        match parsed {
            Parsed::Reference(reference) => scan.references.push(reference),
            Parsed::Invalid(invalid_call) => scan.invalid_calls.push(InvalidReferenceCall {
                start: offsets.advance(text, index),
                ..invalid_call
            }),
        }
        index = next;
    }
    Ok(scan)
}

/// Parse the reference call at `start` as Python `_parse_reference_at` does.
fn parse_reference(
    text: &str,
    start: usize,
    call: &ReferencePrefix,
    syntax: &LexicalSyntax,
) -> Scan<(Parsed, usize)> {
    let sql: &[u8] = text.as_bytes();
    let (prefix, kind) = *call;
    let open = start + prefix.len() - 1;
    let close = matching_paren(sql, open, syntax, REFERENCE_CONTEXT)?;
    let arguments: &str = &text[open + 1..close];
    let call_text: &str = &text[start..=close];
    let names = if kind == DBT_REFERENCE_KIND {
        dbt_reference_names(arguments)
    } else {
        bare_name(arguments).map(|name| (name, None))
    };
    let Some((name, package)) = names else {
        let invalid = invalid_arguments(kind, text, (start, open, close), syntax);
        return Ok((Parsed::Invalid(invalid), close + 1));
    };
    let mut call_argument_count = None;
    let mut call_partitioned = false;
    if kind == TABLE_FUNCTION_REFERENCE_KIND {
        let suffix = python_whitespace_end(text, close + 1);
        if sql.get(suffix) != Some(&b'(') {
            let corrected_call = format!("{}()", example_call(kind, &[name.as_str()]));
            let invalid = InvalidReferenceCall {
                kind,
                call: call_text.to_owned(),
                start: 0,
                message: format!(
                    "{} must be followed by an argument list",
                    function_name(kind)
                ),
                help: format!(
                    "pass the function arguments in a second set of parentheses, using () for \
                     no arguments: {corrected_call}"
                ),
                corrected_call,
            };
            return Ok((Parsed::Invalid(invalid), close + 1));
        }
        let suffix_close = matching_paren(sql, suffix, syntax, TABLE_FUNCTION_CALL_CONTEXT)?;
        call_argument_count = Some(split_arguments(text, suffix + 1, suffix_close, syntax)?.len());
        call_partitioned = partition_clause_follows(text, suffix_close + 1);
    }
    Ok((
        Parsed::Reference(SqlReference {
            kind,
            name,
            package,
            call_argument_count,
            call_partitioned,
        }),
        close + 1,
    ))
}

/// Python `_invalid_reference_arguments`: the P012 message, help and corrected call.
fn invalid_arguments(
    kind: &'static str,
    text: &str,
    (start, open, close): (usize, usize, usize),
    syntax: &LexicalSyntax,
) -> InvalidReferenceCall {
    let allowed: &[usize] = if kind == DBT_REFERENCE_KIND {
        &[1, 2]
    } else {
        &[1]
    };
    let authored: Option<Vec<String>> = authored_names(text, open + 1, close, syntax)
        .filter(|names| allowed.contains(&names.len()));
    let names: Vec<String> = authored.unwrap_or_else(|| {
        PLACEHOLDER_NAMES
            .iter()
            .find(|(candidate, _)| *candidate == kind)
            .map_or_else(Vec::new, |(_, names)| {
                names.iter().map(|name| (*name).to_owned()).collect()
            })
    });
    let name_refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let mut corrected_call = example_call(kind, &name_refs);
    if kind == TABLE_FUNCTION_REFERENCE_KIND {
        corrected_call.push_str("(...)");
    }
    let accepted = if kind == DBT_REFERENCE_KIND {
        "one double-quoted model name, or a double-quoted package name and model name separated \
         by a comma"
    } else {
        "exactly one double-quoted name"
    };
    let call: String = text[start..=close].to_owned();
    let words: Vec<&str> = call
        .split(char::is_whitespace)
        .filter(|word| !word.is_empty())
        .collect();
    let prefix = function_name(kind);
    InvalidReferenceCall {
        kind,
        message: format!("{} is not a valid {prefix}() call", words.join(" ")),
        help: format!(
            "{prefix}() takes {accepted}, with no comments or extra spaces inside the \
             parentheses: {corrected_call}"
        ),
        corrected_call,
        call,
        start: 0,
    }
}

/// Python `_authored_reference_names`: each argument as a plain or quoted name, else `None`.
fn authored_names(
    text: &str,
    start: usize,
    end: usize,
    syntax: &LexicalSyntax,
) -> Option<Vec<String>> {
    let Ok(arguments) = split_arguments(text, start, end, syntax) else {
        return None;
    };
    arguments
        .iter()
        .map(|argument| {
            let bytes: &[u8] = argument.as_bytes();
            let name: &str = match (bytes.first(), bytes.last()) {
                (Some(first), Some(last))
                    if bytes.len() >= QUOTED_NAME_MINIMUM_BYTES
                        && first == last
                        && NAME_QUOTE_BYTES.contains(first) =>
                {
                    &argument[1..argument.len() - 1]
                }
                _ => argument,
            };
            is_authored_name(name.as_bytes()).then(|| name.to_owned())
        })
        .collect()
}

/// Python `[A-Za-z_][A-Za-z0-9_.]*` matched against the whole name.
fn is_authored_name(name: &[u8]) -> bool {
    name.first()
        .is_some_and(|first| first.is_ascii_alphabetic() || *first == b'_')
        && name[1..]
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.'))
}

/// Python `SqlReferenceKind.function_name`.
fn function_name(kind: &str) -> String {
    format!("__{kind}")
}

/// Python `SqlReferenceKind.example_call(*names, quote='"')`.
fn example_call(kind: &str, names: &[&str]) -> String {
    let quoted: Vec<String> = names
        .iter()
        .map(|name| format!("\"{}\"", name.replace('"', "\"\"")))
        .collect();
    format!("{}({})", function_name(kind), quoted.join(", "))
}

/// Python `"([^"]+)"` matched against the whole argument text; `None` when it does not match.
fn bare_name(arguments: &str) -> Option<String> {
    match quoted_name_end(arguments.as_bytes(), 0) {
        Some(end) if end == arguments.len() => Some(arguments[1..end - 1].to_owned()),
        _ => None,
    }
}

/// Python `"([^"]+)"(?:\s*,\s*"([^"]+)")?` over the whole text: name and package, or `None`.
fn dbt_reference_names(arguments: &str) -> Option<(String, Option<String>)> {
    let bytes: &[u8] = arguments.as_bytes();
    let first_end = quoted_name_end(bytes, 0)?;
    let first: String = arguments[1..first_end - 1].to_owned();
    if first_end == arguments.len() {
        return Some((first, None));
    }
    let separator = python_whitespace_end(arguments, first_end);
    if bytes.get(separator) != Some(&b',') {
        return None;
    }
    let second_start = python_whitespace_end(arguments, separator + 1);
    match quoted_name_end(bytes, second_start) {
        Some(end) if end == arguments.len() => {
            Some((arguments[second_start + 1..end - 1].to_owned(), Some(first)))
        }
        _ => None,
    }
}

/// The offset after a `"name"` with at least one character starting at `start`.
fn quoted_name_end(arguments: &[u8], start: usize) -> Option<usize> {
    if arguments.get(start) != Some(&b'"') {
        return None;
    }
    let close = start
        + 1
        + arguments[start + 1..]
            .iter()
            .position(|byte| *byte == b'"')?;
    (close > start + 1).then_some(close + 1)
}

/// Python `_split_top_level_arguments` over `sql[start..end]`: comments read as one space.
fn split_arguments(
    text: &str,
    start: usize,
    end: usize,
    syntax: &LexicalSyntax,
) -> Scan<Vec<String>> {
    let sql: &[u8] = text.as_bytes();
    let mut arguments: Vec<String> = Vec::new();
    let mut current: String = String::new();
    let mut depth = 0isize;
    let mut saw_separator = false;
    let mut index = start;
    while index < end {
        if let Some(non_code) = non_code_end(sql, index, syntax, REFERENCE_CONTEXT)? {
            if QUOTE_BYTES.contains(&sql[index]) {
                current.push_str(&text[index..non_code]);
            } else {
                current.push(' ');
            }
            index = non_code;
            continue;
        }
        match sql[index] {
            b'(' => depth += 1,
            b')' => depth -= 1,
            b',' if depth == 0 => {
                arguments.push(stripped_argument(&current)?);
                current.clear();
                saw_separator = true;
                index += 1;
                continue;
            }
            _ => {}
        }
        let character: char = text[index..].chars().next().unwrap_or_default();
        current.push(character);
        index += character.len_utf8();
    }
    let argument: &str = current.trim_matches(char::is_whitespace);
    if !argument.is_empty() {
        arguments.push(argument.to_owned());
    } else if saw_separator {
        return Err(empty_argument());
    }
    Ok(arguments)
}

/// Python `str.strip()` of one argument, which must not be empty.
fn stripped_argument(argument: &str) -> Scan<String> {
    let stripped: &str = argument.trim_matches(char::is_whitespace);
    if stripped.is_empty() {
        return Err(empty_argument());
    }
    Ok(stripped.to_owned())
}

fn empty_argument() -> String {
    format!("{REFERENCE_CONTEXT} contains an empty argument")
}

/// Whether `OVER (PARTITION BY` follows `start`, in any case, with whitespace or comments between.
fn partition_clause_follows(text: &str, start: usize) -> bool {
    let Some(open) = over_clause_open(text, start) else {
        return false;
    };
    let mut position: usize = open + 1;
    for word in PARTITION_CLAUSE_WORDS {
        let Some(word_start) = code_start(text, position) else {
            return false;
        };
        position = word_start;
        let Some(candidate) = text.get(position..position + word.len()) else {
            return false;
        };
        if !candidate.eq_ignore_ascii_case(word) {
            return false;
        }
        position += word.len();
        if text[position..]
            .chars()
            .next()
            .is_some_and(|character| character.is_alphanumeric() || character == '_')
        {
            return false;
        }
    }
    true
}

/// Python `_skip_whitespace`, which skips `str.isspace()` characters.
fn python_whitespace_end(text: &str, start: usize) -> usize {
    text[start..]
        .char_indices()
        .find(|(_, character)| !char::is_whitespace(*character))
        .map_or(text.len(), |(offset, _)| start + offset)
}

fn non_code_end(
    sql: &[u8],
    index: usize,
    syntax: &LexicalSyntax,
    context: &str,
) -> Scan<Option<usize>> {
    if !NON_CODE_START[usize::from(sql[index])] {
        return Ok(None);
    }
    dialect_non_code_end(sql, index, syntax).map_err(|unclosed| unclosed_error(context, unclosed))
}

/// Python `find_matching_paren`, which only looks for comments and quotes at special characters.
fn matching_paren(sql: &[u8], open: usize, syntax: &LexicalSyntax, context: &str) -> Scan<usize> {
    let mut depth = 1usize;
    let mut index = open + 1;
    while index < sql.len() {
        match sql[index] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Ok(index);
                }
            }
            _ => {
                if let Some(end) = non_code_end(sql, index, syntax, context)? {
                    index = end;
                    continue;
                }
            }
        }
        index += 1;
    }
    Err(unclosed_error(context, Unclosed::Parenthesis))
}

fn unclosed_error(context: &str, unclosed: Unclosed) -> String {
    let construct = match unclosed {
        Unclosed::BlockComment => "block comment",
        Unclosed::Quote => "quoted string",
        Unclosed::Parenthesis => "parenthesis",
    };
    format!("{context} contains an unclosed {construct}")
}

/// Converts increasing byte offsets of UTF-8 text into Python code-point offsets.
#[derive(Default)]
struct CharOffsets {
    byte: usize,
    char: usize,
}

impl CharOffsets {
    fn advance(&mut self, text: &str, byte: usize) -> usize {
        self.char += text[self.byte..byte].chars().count();
        self.byte = byte;
        self.char
    }
}
