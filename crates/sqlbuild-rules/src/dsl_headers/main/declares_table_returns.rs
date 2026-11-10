//! `declares_table_returns`: whether a function header returns a table.

use sqlbuild_core::text::main::python_isspace::python_isspace;

use crate::dsl_headers::_helpers::quoted::quoted_value_end;
use crate::dsl_headers::_helpers::text::is_word;
use crate::dsl_headers::constants::{QUOTES, RETURNS_KEYWORD, TABLE_RETURNS_KEYWORD};
use crate::dsl_headers::main::inner_header_text::inner_header_text;

/// The index after `keyword` at `start`, matched without case, when a word ends there.
fn keyword_end(text: &[char], start: usize, keyword: &str) -> Option<usize> {
    let end: usize = start + keyword.chars().count();
    let candidate: &[char] = text.get(start..end)?;
    let matched: bool = candidate
        .iter()
        .zip(keyword.chars())
        .all(|(left, right)| left.eq_ignore_ascii_case(&right));
    (matched && !text.get(end).copied().is_some_and(is_word)).then_some(end)
}

/// The first index at or after `start` that is not whitespace.
fn after_space(text: &[char], start: usize) -> usize {
    text[start..]
        .iter()
        .position(|character| !python_isspace(*character))
        .map_or(text.len(), |offset| start + offset)
}

/// Whether the header's top-level `returns` value is `table (...)`, outside quoted text.
pub fn declares_table_returns(header_text: &[char]) -> bool {
    let inner: &[char] = inner_header_text(header_text);
    let mut depth: usize = 0;
    let mut entry_start: bool = true;
    let mut index: usize = 0;
    while index < inner.len() {
        let character: char = inner[index];
        if QUOTES.contains(&character) {
            index = quoted_value_end(inner, index);
            entry_start = false;
            continue;
        }
        if depth == 0 && entry_start && !python_isspace(character) {
            entry_start = false;
            if let Some(value) = keyword_end(inner, index, RETURNS_KEYWORD) {
                return keyword_end(inner, after_space(inner, value), TABLE_RETURNS_KEYWORD)
                    .is_some_and(|table| inner.get(after_space(inner, table)) == Some(&'('));
            }
        }
        match character {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => entry_start = true,
            _ => {}
        }
        index += 1;
    }
    false
}
