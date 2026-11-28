//! Line-leading parenthesized DSL regions, found past comments and SQL strings.

use sqlbuild_core::text::main::python_isspace::python_isspace;
use sqlbuild_sqltext::header_strings::models::DescriptionKeys;

use crate::dsl_headers::_helpers::text::{find, is_word};
use crate::dsl_headers::constants::QUOTES;
use crate::dsl_headers::models::HeaderSpan;

/// The end of the SQL string quoted by `quote` that opens at `start`; `''` escapes a quote.
fn sql_string_end(text: &[char], start: usize, quote: char) -> usize {
    let mut index: usize = start + 1;
    while index < text.len() {
        if text[index] == quote {
            if text.get(index + 1) == Some(&quote) {
                index += 2;
                continue;
            }
            return index + 1;
        }
        index += 1;
    }
    text.len()
}

/// The end of the comment or string at `index`, if one starts there.
fn skipped_end(text: &[char], index: usize) -> Option<usize> {
    let next: Option<&char> = text.get(index + 1);
    match text[index] {
        '-' if next == Some(&'-') => Some(
            text[index + 2..]
                .iter()
                .position(|character| *character == '\n')
                .map_or(text.len(), |offset| index + 2 + offset + 1),
        ),
        '/' if next == Some(&'*') => {
            Some(find(text, &['*', '/'], index + 2).map_or(text.len(), |found| found + 2))
        }
        quote @ ('\'' | '"') => Some(sql_string_end(text, index, quote)),
        _ => None,
    }
}

/// The kind keyword that starts the line at `index`, after spaces, and its offset.
fn leading_kind<'k>(text: &[char], index: usize, kinds: &[&'k str]) -> Option<(&'k str, usize)> {
    if index > 0 && text[index - 1] != '\n' {
        return None;
    }
    let mut start: usize = index;
    while start < text.len()
        && python_isspace(text[start])
        && text[start] != '\r'
        && text[start] != '\n'
    {
        start += 1;
    }
    kinds.iter().find_map(|kind| {
        let mut end: usize = start;
        for expected in kind.chars() {
            if text.get(end) != Some(&expected) {
                return None;
            }
            end += 1;
        }
        (!text.get(end).is_some_and(|next| is_word(*next))).then_some((*kind, start))
    })
}

/// The span of the parenthesized header body after `body_start`, through an optional `;`.
fn header_end(text: &[char], body_start: usize) -> Option<usize> {
    let mut index: usize = skip_whitespace(text, body_start);
    if text.get(index) != Some(&'(') {
        return None;
    }
    let mut depth: i64 = 0;
    let mut description_keys: DescriptionKeys<'_, char> = DescriptionKeys::new(text);
    loop {
        index = (index..text.len())
            .find(|position| matches!(text[*position], '(' | ')' | '\'' | '"'))?;
        let character: char = text[index];
        if QUOTES.contains(&character) {
            index = description_keys.quoted_end(index)?;
            continue;
        }
        if character == '(' {
            depth += 1;
        } else {
            depth -= 1;
            if depth == 0 {
                let terminator: usize = skip_whitespace(text, index + 1);
                if text.get(terminator) == Some(&';') {
                    return Some(terminator + 1);
                }
                return Some(index + 1);
            }
        }
        index += 1;
    }
}

pub(crate) fn skip_whitespace(text: &[char], mut index: usize) -> usize {
    while index < text.len() && python_isspace(text[index]) {
        index += 1;
    }
    index
}

/// The first non-space offset in `start..end`, or `end`.
pub(crate) fn first_non_whitespace(text: &[char], start: usize, end: usize) -> usize {
    (start..end)
        .find(|index| !python_isspace(text[*index]))
        .unwrap_or(end)
}

/// Line-leading parenthesized regions named by one of `kinds`, sorted by name.
pub(crate) fn delimited_regions(
    text: &[char],
    kinds: &[&str],
    first_only: bool,
) -> Vec<HeaderSpan> {
    let mut sorted: Vec<&str> = kinds.to_vec();
    sorted.sort_unstable();
    let mut spans: Vec<HeaderSpan> = Vec::new();
    let mut covered_until: usize = 0;
    let mut index: usize = 0;
    while index < text.len() {
        if let Some(end) = skipped_end(text, index) {
            index = end;
            continue;
        }
        let Some((kind, keyword_start)) = leading_kind(text, index, &sorted) else {
            index += 1;
            continue;
        };
        let body_start: usize = keyword_start + kind.chars().count();
        index = body_start;
        if keyword_start < covered_until {
            continue;
        }
        if let Some(end) = header_end(text, body_start) {
            spans.push(HeaderSpan {
                kind: kind.to_owned(),
                start: keyword_start,
                end,
            });
            covered_until = end;
            if first_only {
                break;
            }
        }
    }
    spans
}
