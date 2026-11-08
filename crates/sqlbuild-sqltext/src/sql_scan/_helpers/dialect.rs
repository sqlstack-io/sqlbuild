//! Dialect-aware comment and quoted-text boundaries, mirroring Python `dialect_non_code_end`.

use crate::sql_scan::_helpers::dollar::{continues_dollar_word, dollar_quote_end};
use crate::sql_scan::_helpers::parens::matching_paren_with;
use crate::sql_scan::constants::{
    STRING_PREFIX_CHARACTERS, STRING_PREFIX_MAX_LENGTH, TRIPLE_QUOTE_LENGTH,
};
use crate::sql_scan::models::{LexicalSyntax, Unclosed};

/// Return the end of a comment or quoted text starting at `index` under the dialect's rules.
pub(crate) fn non_code_end(
    sql: &[u8],
    index: usize,
    syntax: &LexicalSyntax,
) -> Result<Option<usize>, Unclosed> {
    if sql
        .get(index)
        .is_some_and(|first| !may_open_non_code(*first, syntax))
    {
        return Ok(None);
    }
    if sql[index..].starts_with(b"/*") {
        return block_comment_end(sql, index, syntax).map(Some);
    }
    if syntax
        .line_comment_prefixes
        .iter()
        .any(|prefix| sql[index..].starts_with(prefix.as_bytes()))
    {
        return Ok(Some(
            sql[index..]
                .iter()
                .position(|byte| *byte == b'\n')
                .map_or(sql.len(), |offset| index + offset + 1),
        ));
    }
    match sql.get(index) {
        Some(b'$') => dollar_quote_end(sql, index),
        Some(b'\'' | b'"' | b'`') => quoted_text_end(sql, index, syntax).map(Some),
        Some(b'[') if syntax.bracket_identifiers => bracket_identifier_end(sql, index).map(Some),
        _ => Ok(None),
    }
}

/// Whether a comment or quoted text may open with `byte`; most code bytes are rejected here cheaply.
fn may_open_non_code(byte: u8, syntax: &LexicalSyntax) -> bool {
    matches!(byte, b'/' | b'$' | b'\'' | b'"' | b'`')
        || (byte == b'[' && syntax.bracket_identifiers)
        || syntax
            .line_comment_prefixes
            .iter()
            .any(|prefix| prefix.as_bytes().first().is_none_or(|start| *start == byte))
}

/// Return every comment and quoted-text range in `sql`; an unclosed one extends to the end.
pub(crate) fn non_code_ranges(sql: &str, syntax: &LexicalSyntax) -> Vec<(usize, usize)> {
    let bytes = sql.as_bytes();
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        match non_code_end(bytes, index, syntax) {
            Ok(Some(end)) => {
                ranges.push((index, end));
                index = end;
            }
            Ok(None) => index += 1,
            Err(_) => {
                ranges.push((index, bytes.len()));
                break;
            }
        }
    }
    ranges
}

/// Return the index of the parenthesis closing the one at `open` under the dialect's rules.
pub(crate) fn matching_paren(
    sql: &[u8],
    open: usize,
    syntax: &LexicalSyntax,
) -> Result<usize, Unclosed> {
    matching_paren_with(sql, open, |sql, index| non_code_end(sql, index, syntax))
}

fn block_comment_end(sql: &[u8], start: usize, syntax: &LexicalSyntax) -> Result<usize, Unclosed> {
    let mut depth = 1usize;
    let mut index = start + 2;
    while depth > 0 {
        let close = find(sql, b"*/", index).ok_or(Unclosed::BlockComment)?;
        let open = if syntax.nested_block_comments {
            find(&sql[..close], b"/*", index)
        } else {
            None
        };
        if let Some(open) = open {
            depth += 1;
            index = open + 2;
        } else {
            depth -= 1;
            index = close + 2;
        }
    }
    Ok(index)
}

fn quoted_text_end(sql: &[u8], start: usize, syntax: &LexicalSyntax) -> Result<usize, Unclosed> {
    let quote = sql[start];
    let prefix = string_literal_prefix(sql, start);
    let mut backslash_escapes = syntax.backslash_escapes(quote)
        || (syntax.escape_string_prefix && quote == b'\'' && prefix == b"e");
    if syntax.raw_string_prefix && prefix.contains(&b'r') {
        backslash_escapes = false;
    }
    let triple = [quote; TRIPLE_QUOTE_LENGTH];
    let delimiter: &[u8] = if syntax.triple_quoted_strings && sql[start..].starts_with(&triple) {
        &triple
    } else {
        &triple[..1]
    };
    let mut index = start + delimiter.len();
    while index < sql.len() {
        if backslash_escapes && sql[index] == b'\\' {
            index += 2;
            continue;
        }
        if sql[index..].starts_with(delimiter) {
            if delimiter.len() == 1 && sql.get(index + 1) == Some(&quote) {
                index += 2;
                continue;
            }
            return Ok(index + delimiter.len());
        }
        index += 1;
    }
    Err(Unclosed::Quote)
}

/// The end of the bracket identifier opened at `start`, where `]]` is one `]`.
fn bracket_identifier_end(sql: &[u8], start: usize) -> Result<usize, Unclosed> {
    let mut index = start + 1;
    while let Some(offset) = sql[index..].iter().position(|byte| *byte == b']') {
        let close = index + offset;
        if sql.get(close + 1) != Some(&b']') {
            return Ok(close + 1);
        }
        index = close + 2;
    }
    Err(Unclosed::Quote)
}

fn string_literal_prefix(sql: &[u8], start: usize) -> Vec<u8> {
    let mut prefix_start = start;
    while prefix_start > 0
        && start - prefix_start < STRING_PREFIX_MAX_LENGTH
        && STRING_PREFIX_CHARACTERS.contains(&sql[prefix_start - 1])
    {
        prefix_start -= 1;
    }
    if prefix_start > 0 && continues_dollar_word(sql[prefix_start - 1]) {
        return Vec::new();
    }
    sql[prefix_start..start].to_ascii_lowercase()
}

fn find(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    haystack
        .get(from..)?
        .windows(needle.len())
        .position(|window| window == needle)
        .map(|offset| from + offset)
}
