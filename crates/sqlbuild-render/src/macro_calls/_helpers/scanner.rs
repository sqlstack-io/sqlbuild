//! Byte offset port of the Python macro call scanner, with the errors Python's scan raises.

use sqlbuild_core::text::main::is_letter::is_letter;
use sqlbuild_sqltext::sql_scan::main::dialect_non_code_end::dialect_non_code_end;
use sqlbuild_sqltext::sql_scan::models::{LexicalSyntax, Unclosed};

use crate::macro_calls::errors::ScanError;
use crate::macro_calls::main::opens_bare_call::opens_bare_call;

/// The prefix of a Python macro call after its `@`.
const PYTHON_PREFIX: &str = "py.";
/// The line comment every scan honours, the only one inside `@py.` call arguments.
const GENERIC_LINE_COMMENT_PREFIX: &str = "--";
/// The prefix of a SQL macro call after its `@`.
const SQL_PREFIX: &str = "sql.";
/// Declaration and SQL test parameter references, which scans read as calls that need no kind.
const DECLARATION_NAMES: [&str; 3] = ["const", "enum", "param"];

const TYPED_REFERENCE_NAMES: [&str; 3] = ["__ref", "__source", "__seed"];

/// One call found by the scanner, in byte offsets of the scanned text.
pub(crate) struct ScannedCall {
    pub(crate) start: usize,
    pub(crate) open: usize,
    pub(crate) close: usize,
    pub(crate) name: String,
    /// A `@py.` call, whose arguments are Python literals rather than SQL.
    pub(crate) python: bool,
}

/// Where Python's scan of the text raises: in the call starting at a byte offset, or between calls.
pub(crate) struct ScannedFailure {
    pub(crate) call_start: Option<usize>,
    pub(crate) error: ScanError,
}

/// Every top-level Python macro call of `text` in order, up to the first error the scan raises.
pub(crate) fn top_level_calls(
    text: &str,
    syntax: Option<&LexicalSyntax>,
) -> (Vec<ScannedCall>, Option<ScannedFailure>) {
    top_level_calls_of(text, false, syntax)
}

/// Every top-level Python or SQL macro call of `text`, as static name scans read them.
pub(crate) fn top_level_named_calls(
    text: &str,
    syntax: Option<&LexicalSyntax>,
) -> (Vec<ScannedCall>, Option<ScannedFailure>) {
    top_level_calls_of(text, true, syntax)
}

fn top_level_calls_of(
    text: &str,
    with_sql: bool,
    syntax: Option<&LexicalSyntax>,
) -> (Vec<ScannedCall>, Option<ScannedFailure>) {
    let mut calls: Vec<ScannedCall> = Vec::new();
    let mut cursor = 0;
    while cursor < text.len() {
        let start = match next_macro_start(text, cursor, syntax) {
            Ok(Some(start)) => start,
            Ok(None) => break,
            Err(error) => {
                let failure = ScannedFailure {
                    call_start: None,
                    error,
                };
                return (calls, Some(failure));
            }
        };
        match call_at(text, start, with_sql, syntax) {
            Ok(call) => {
                cursor = call.close + 1;
                calls.push(call);
            }
            Err(error) => {
                let failure = ScannedFailure {
                    call_start: Some(start),
                    error,
                };
                return (calls, Some(failure));
            }
        }
    }
    (calls, None)
}

/// What starts at an `@`: a macro call of either kind, a declaration, a bare call, or none.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CallStart {
    Python,
    Sql,
    Declaration,
    Bare,
    None,
}

/// What the `@` at byte `at` starts.
pub(crate) fn call_start_at(text: &str, at: usize) -> CallStart {
    let rest: &str = &text[at + 1..];
    if rest.starts_with(PYTHON_PREFIX) && name_call_at(text, at + 1 + PYTHON_PREFIX.len()) {
        return CallStart::Python;
    }
    if rest.starts_with(SQL_PREFIX) && name_call_at(text, at + 1 + SQL_PREFIX.len()) {
        return CallStart::Sql;
    }
    if name_call_at(text, at + 1) {
        let name_end: usize = identifier_end(text, at + 1);
        if DECLARATION_NAMES.contains(&&text[at + 1..name_end]) {
            return CallStart::Declaration;
        }
        if opens_bare_call(text, name_end) {
            return CallStart::Bare;
        }
    }
    CallStart::None
}

/// The byte offset where the called name starts, after the `@` and any kind prefix.
fn name_start(text: &str, at: usize) -> usize {
    match call_start_at(text, at) {
        CallStart::Python => at + 1 + PYTHON_PREFIX.len(),
        CallStart::Sql => at + 1 + SQL_PREFIX.len(),
        CallStart::Declaration | CallStart::Bare | CallStart::None => at + 1,
    }
}

/// The error a call start raises: a bare call always, a SQL macro call unless `with_sql`.
fn kind_error(start: CallStart, with_sql: bool) -> Option<ScanError> {
    match start {
        CallStart::Bare => Some(ScanError::BareCall),
        CallStart::Sql if !with_sql => Some(ScanError::SqlCall),
        CallStart::Sql | CallStart::Python | CallStart::Declaration | CallStart::None => None,
    }
}

/// Python's `_parse_macro_name`: the identifier after the `@` at byte `start`.
pub(crate) fn call_name(text: &str, start: usize) -> String {
    let from: usize = name_start(text, start);
    text[from..identifier_end(text, from)].to_owned()
}

/// Every Python or SQL macro call start's name in order, nested calls included.
pub(crate) fn every_call_name(
    text: &str,
    syntax: Option<&LexicalSyntax>,
) -> Result<Vec<String>, ScanError> {
    let mut names: Vec<String> = Vec::new();
    let mut cursor = 0;
    while cursor < text.len() {
        let Some(start) = next_macro_start(text, cursor, syntax)? else {
            break;
        };
        if let Some(error) = kind_error(call_start_at(text, start), true) {
            return Err(error);
        }
        let from: usize = name_start(text, start);
        let name_end = identifier_end(text, from);
        names.push(text[from..name_end].to_owned());
        let open = skip_whitespace(text, name_end);
        cursor = open + char_at(text, open).map_or(1, char::len_utf8);
    }
    Ok(names)
}

/// Names of every call nested in `args` in source order, in one pass without recursion.
pub(crate) fn nested_names(
    args: &str,
    syntax: Option<&LexicalSyntax>,
) -> Result<Vec<String>, ScanError> {
    let bytes: &[u8] = args.as_bytes();
    let mut names: Vec<String> = Vec::new();
    let mut open_call_depths: Vec<usize> = Vec::new();
    let mut depth: usize = 0;
    let mut index: usize = 0;
    while index < bytes.len() {
        if let Some(end) = non_code_end(bytes, index, syntax)? {
            index = end;
            continue;
        }
        match bytes[index] {
            b'@' if macro_call_starts_at(args, index) => {
                if let Some(error) = kind_error(call_start_at(args, index), false) {
                    return Err(error);
                }
                let (name, open) = call_header(args, index)?;
                names.push(name);
                depth += 1;
                open_call_depths.push(depth);
                index = open + 1;
            }
            b'(' => {
                depth += 1;
                index += 1;
            }
            b')' => {
                if open_call_depths.last() == Some(&depth) {
                    let _ = open_call_depths.pop();
                }
                depth = depth.checked_sub(1).ok_or(ScanError::UnclosedParenthesis)?;
                index += 1;
            }
            _ => index += 1,
        }
    }
    if open_call_depths.is_empty() {
        Ok(names)
    } else {
        Err(ScanError::UnclosedParenthesis)
    }
}

/// Whether call arguments mention a typed reference function name anywhere.
pub(crate) fn mentions_typed_reference(args: &str) -> bool {
    TYPED_REFERENCE_NAMES.iter().any(|name| args.contains(name))
}

fn call_at(
    text: &str,
    start: usize,
    with_sql: bool,
    syntax: Option<&LexicalSyntax>,
) -> Result<ScannedCall, ScanError> {
    let kind: CallStart = call_start_at(text, start);
    if let Some(error) = kind_error(kind, with_sql) {
        return Err(error);
    }
    let (name, open) = call_header(text, start)?;
    let python: bool = kind == CallStart::Python;
    let python_syntax: LexicalSyntax = python_argument_syntax();
    let argument_syntax: Option<&LexicalSyntax> =
        if python { Some(&python_syntax) } else { syntax };
    let close = matching_paren(text.as_bytes(), open, argument_syntax)?;
    Ok(ScannedCall {
        start,
        open,
        close,
        name,
        python,
    })
}

/// Python's string rules for `@py.` call arguments, with only the generic `--` line comment.
pub(crate) fn python_argument_syntax() -> LexicalSyntax {
    LexicalSyntax {
        backslash_escape_quotes: vec!["'".to_owned(), "\"".to_owned()],
        raw_string_prefix: true,
        triple_quoted_strings: true,
        line_comment_prefixes: vec![GENERIC_LINE_COMMENT_PREFIX.to_owned()],
        ..LexicalSyntax::default()
    }
}

fn call_header(text: &str, start: usize) -> Result<(String, usize), ScanError> {
    let from: usize = name_start(text, start);
    let name_end = identifier_end(text, from);
    let open = skip_whitespace(text, name_end);
    if text.as_bytes().get(open) != Some(&b'(') {
        return Err(ScanError::MissingParenthesis);
    }
    Ok((text[from..name_end].to_owned(), open))
}

fn next_macro_start(
    text: &str,
    from: usize,
    syntax: Option<&LexicalSyntax>,
) -> Result<Option<usize>, ScanError> {
    let bytes = text.as_bytes();
    let mut index = from;
    while index < bytes.len() {
        if let Some(end) = non_code_end(bytes, index, syntax)? {
            index = end;
            continue;
        }
        match bytes[index] {
            b'@' => {
                if macro_call_starts_at(text, index) {
                    return Ok(Some(index));
                }
                index += 1;
            }
            _ => index += 1,
        }
    }
    Ok(None)
}

fn macro_call_starts_at(text: &str, at: usize) -> bool {
    call_start_at(text, at) != CallStart::None
}

/// Whether an identifier starting at byte `from` runs, as a call name, to an opening parenthesis.
fn name_call_at(text: &str, from: usize) -> bool {
    let at: usize = from - 1;
    let Some(first) = char_at(text, at + 1) else {
        return false;
    };
    if !(first == '_' || is_letter(first)) {
        return false;
    }
    let mut cursor = at + 1 + first.len_utf8();
    while let Some(character) = char_at(text, cursor) {
        if !is_identifier_continue(character) {
            break;
        }
        cursor = skip_whitespace(text, cursor + character.len_utf8());
    }
    text.as_bytes().get(cursor) == Some(&b'(')
}

fn identifier_end(text: &str, from: usize) -> usize {
    let mut cursor = from;
    while let Some(character) = char_at(text, cursor) {
        if !is_identifier_continue(character) {
            break;
        }
        cursor += character.len_utf8();
    }
    cursor
}

fn is_identifier_continue(character: char) -> bool {
    character == '_' || char::is_alphanumeric(character)
}

fn skip_whitespace(text: &str, from: usize) -> usize {
    let mut cursor = from;
    while let Some(character) = char_at(text, cursor) {
        if !char::is_whitespace(character) {
            break;
        }
        cursor += character.len_utf8();
    }
    cursor
}

fn char_at(text: &str, index: usize) -> Option<char> {
    text.get(index..).and_then(|rest| rest.chars().next())
}

fn matching_paren(
    bytes: &[u8],
    open: usize,
    syntax: Option<&LexicalSyntax>,
) -> Result<usize, ScanError> {
    let mut depth = 1usize;
    let mut index = open + 1;
    while index < bytes.len() {
        if let Some(end) = non_code_end(bytes, index, syntax)? {
            index = end;
            continue;
        }
        match bytes[index] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Ok(index);
                }
            }
            _ => {}
        }
        index += 1;
    }
    Err(ScanError::UnclosedParenthesis)
}

/// The end of the comment or quoted text at `index`, under `syntax` or the generic rules.
fn non_code_end(
    bytes: &[u8],
    index: usize,
    syntax: Option<&LexicalSyntax>,
) -> Result<Option<usize>, ScanError> {
    if let Some(syntax) = syntax {
        return dialect_non_code_end(bytes, index, syntax).map_err(|unclosed| match unclosed {
            Unclosed::BlockComment => ScanError::UnclosedComment,
            Unclosed::Quote => ScanError::UnclosedQuote,
            Unclosed::Parenthesis => ScanError::UnclosedParenthesis,
        });
    }
    match bytes[index] {
        b'\'' | b'"' | b'`' => quote_end(bytes, index).map(Some),
        b'$' => dollar_quote_end(bytes, index),
        b'-' if bytes.get(index + 1) == Some(&b'-') => Ok(Some(line_comment_end(bytes, index))),
        b'/' if bytes.get(index + 1) == Some(&b'*') => block_comment_end(bytes, index).map(Some),
        _ => Ok(None),
    }
}

fn quote_end(bytes: &[u8], start: usize) -> Result<usize, ScanError> {
    let quote = bytes[start];
    let doubled_escapes = quote == b'\'' || quote == b'"';
    let mut index = start + 1;
    loop {
        let offset = bytes[index.min(bytes.len())..]
            .iter()
            .position(|byte| *byte == quote)
            .ok_or(ScanError::UnclosedQuote)?;
        index += offset;
        if doubled_escapes && bytes.get(index + 1) == Some(&quote) {
            index += 2;
            continue;
        }
        return Ok(index + 1);
    }
}

fn dollar_quote_end(bytes: &[u8], start: usize) -> Result<Option<usize>, ScanError> {
    if start
        .checked_sub(1)
        .is_some_and(|previous| continues_dollar_word(bytes[previous]))
    {
        return Ok(None);
    }
    let tag_end = bytes[start + 1..]
        .iter()
        .position(|byte| !(byte.is_ascii_alphanumeric() || *byte == b'_'))
        .map_or(bytes.len(), |offset| start + 1 + offset);
    let tag = &bytes[start + 1..tag_end];
    if bytes.get(tag_end) != Some(&b'$') || tag.first().is_some_and(u8::is_ascii_digit) {
        return Ok(None);
    }
    let delimiter = &bytes[start..=tag_end];
    bytes[tag_end + 1..]
        .windows(delimiter.len())
        .position(|window| window == delimiter)
        .map(|offset| Some(tag_end + 1 + offset + delimiter.len()))
        .ok_or(ScanError::UnclosedQuote)
}

fn continues_dollar_word(byte: u8) -> bool {
    !byte.is_ascii() || byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$')
}

fn line_comment_end(bytes: &[u8], start: usize) -> usize {
    bytes[start..]
        .iter()
        .position(|byte| *byte == b'\n')
        .map_or(bytes.len(), |offset| start + offset + 1)
}

fn block_comment_end(bytes: &[u8], start: usize) -> Result<usize, ScanError> {
    bytes[start + 2..]
        .windows(2)
        .position(|pair| pair == b"*/")
        .map(|offset| start + 2 + offset + 2)
        .ok_or(ScanError::UnclosedComment)
}
