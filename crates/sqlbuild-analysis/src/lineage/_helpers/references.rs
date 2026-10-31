//! Python's `_normalize_sqlbuild_refs`, matching its backreferencing patterns exactly.

use crate::lineage::constants::{
    PHYSICAL_NAME_REPLACEMENT, PHYSICAL_RESOURCE_PREFIX, REFERENCE_PREFIX, UDF_CALL,
};
use crate::lineage::models::{LineageResourceType, PhysicalResource};

const REFERENCE_KINDS: [(&str, LineageResourceType); 3] = [
    ("ref(", LineageResourceType::Model),
    ("source(", LineageResourceType::Source),
    ("seed(", LineageResourceType::Seed),
];

/// The resource references in `sql`, in match order, repeats included.
pub(crate) fn physical_resources(sql: &str) -> Vec<PhysicalResource> {
    let mut resources: Vec<PhysicalResource> = Vec::new();
    scan_calls(sql, reference_call, |_, (resource_type, name)| {
        resources.push(PhysicalResource {
            resource_type,
            resource_name: name.to_owned(),
            physical_name: physical_resource_name(resource_type, name),
        });
    });
    resources
}

/// `sql` with every reference and UDF call replaced by its physical name.
pub(crate) fn normalized_sql(sql: &str) -> String {
    let references = replace_calls(sql, reference_call, |(resource_type, name)| {
        physical_resource_name(resource_type, name)
    });
    replace_calls(&references, udf_call, |name| {
        format!("{}(", physical_name_text(name))
    })
}

/// Python's `_physical_resource_name`.
pub(crate) fn physical_resource_name(resource_type: LineageResourceType, name: &str) -> String {
    format!(
        "{PHYSICAL_RESOURCE_PREFIX}{}__{}",
        resource_type.as_str(),
        physical_name_text(name)
    )
}

fn physical_name_text(name: &str) -> String {
    let mut text = String::with_capacity(name.len());
    for character in name.chars() {
        if character.is_ascii_alphanumeric() || character == '_' {
            text.push(character);
        } else {
            text.push_str(PHYSICAL_NAME_REPLACEMENT);
        }
    }
    text
}

fn replace_calls<'a, T>(
    sql: &'a str,
    matcher: fn(&'a str) -> Option<(usize, T)>,
    mut replacement: impl FnMut(T) -> String,
) -> String {
    let mut text = String::with_capacity(sql.len());
    let mut copied = 0;
    scan_calls(sql, matcher, |span, value| {
        text.push_str(&sql[copied..span.0]);
        text.push_str(&replacement(value));
        copied = span.1;
    });
    text.push_str(&sql[copied..]);
    text
}

/// Python's `finditer`: non-overlapping matches, left to right. Every match starts with `__`.
fn scan_calls<'a, T>(
    sql: &'a str,
    matcher: fn(&'a str) -> Option<(usize, T)>,
    mut on_match: impl FnMut((usize, usize), T),
) {
    let mut position = 0;
    while let Some(offset) = sql[position..].find(REFERENCE_PREFIX) {
        let start = position + offset;
        match matcher(&sql[start..]) {
            Some((length, value)) => {
                on_match((start, start + length), value);
                position = start + length;
            }
            None => position = start + 1,
        }
    }
}

/// `__(ref|source|seed)\(\s*(['"])(?P<name>[^'"]+)\2\s*\)` at the start of `text`.
fn reference_call(text: &str) -> Option<(usize, (LineageResourceType, &str))> {
    let rest = &text[REFERENCE_PREFIX.len()..];
    let (keyword, resource_type) = REFERENCE_KINDS
        .iter()
        .find(|(keyword, _)| rest.starts_with(keyword))?;
    let opened = REFERENCE_PREFIX.len() + keyword.len();
    let (end, name) = quoted_argument(text, opened)?;
    Some((end, (*resource_type, name)))
}

/// `__udf\(\s*(['"])(?P<name>[^'"]+)\1\s*\)\s*\(` at the start of `text`.
fn udf_call(text: &str) -> Option<(usize, &str)> {
    if !text.starts_with(UDF_CALL) {
        return None;
    }
    let (closed, name) = quoted_argument(text, UDF_CALL.len())?;
    let opened = skip_spaces(text, closed);
    text[opened..]
        .starts_with('(')
        .then_some((opened + 1, name))
}

/// `\s*(['"])(?P<name>[^'"]+)\1\s*\)` from `start`; returns the end after `)` and the name.
fn quoted_argument(text: &str, start: usize) -> Option<(usize, &str)> {
    let quote_start = skip_spaces(text, start);
    let quote = text[quote_start..].chars().next()?;
    if quote != '\'' && quote != '"' {
        return None;
    }
    let name_start = quote_start + 1;
    let name_length = text[name_start..].find(['\'', '"'])?;
    if name_length == 0 || !text[name_start + name_length..].starts_with(quote) {
        return None;
    }
    let close = skip_spaces(text, name_start + name_length + 1);
    text[close..]
        .starts_with(')')
        .then_some((close + 1, &text[name_start..name_start + name_length]))
}

fn skip_spaces(text: &str, start: usize) -> usize {
    text[start..]
        .char_indices()
        .find(|(_, character)| !char::is_whitespace(*character))
        .map_or(text.len(), |(offset, _)| start + offset)
}
