//! Snowflake table function markers wrapped in `TABLE( )`, as SQLBuild renders them.

use std::cmp::Reverse;

use sqlbuild_sqltext::sql_scan::constants::TABLE_WRAPPER;
use sqlbuild_sqltext::sql_scan::main::dialect_matching_paren::dialect_matching_paren;
use sqlbuild_sqltext::sql_scan::main::follows_table_wrapper::follows_table_wrapper;
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

use crate::relation_markers::constants::{MARKER_NAME_PREFIX, TABLE_FUNCTION_MARKER};
use crate::relation_markers::types::MarkerCall;

/// `named` with each table function marker call in `TABLE( )`, written over padding so later text keeps its column.
pub(crate) fn table_wrapped(named: &str, calls: &[MarkerCall], syntax: &LexicalSyntax) -> String {
    let bytes = named.as_bytes();
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    for (index, (function, _)) in calls.iter().enumerate() {
        if function != TABLE_FUNCTION_MARKER {
            continue;
        }
        let name = format!("{MARKER_NAME_PREFIX}{index}");
        let Some(start) = name_start(bytes, &name) else {
            continue;
        };
        let name_end = start + name.len();
        let open = name_end
            + bytes[name_end..]
                .iter()
                .take_while(|byte| byte.is_ascii_whitespace())
                .count();
        if bytes.get(open) != Some(&b'(') || follows_table_wrapper(&bytes[..start]) {
            continue;
        }
        let Ok(close) = dialect_matching_paren(bytes, open, syntax) else {
            continue;
        };
        let spaces = bytes[name_end..]
            .iter()
            .take_while(|byte| **byte == b' ')
            .count();
        let after = close + 1;
        let replaced = usize::from(matches!(bytes.get(after), Some(b' ' | b'\t')));
        let text_follows = !matches!(bytes.get(after), None | Some(b'\n' | b'\r'));
        let borrowed = spaces.min(TABLE_WRAPPER.len() + usize::from(text_follows) - replaced);
        edits.push((start, name_end + borrowed, format!("{TABLE_WRAPPER}{name}")));
        edits.push((after, after + replaced, ")".to_owned()));
    }
    edits.sort_by_key(|(start, _, _)| Reverse(*start));
    let mut wrapped = named.to_owned();
    for (start, end, replacement) in edits {
        wrapped.replace_range(start..end, &replacement);
    }
    wrapped
}

/// Where `name` first stands as a whole word in `bytes`.
fn name_start(bytes: &[u8], name: &str) -> Option<usize> {
    let name = name.as_bytes();
    let word = |byte: Option<&u8>| {
        byte.is_some_and(|value| value.is_ascii_alphanumeric() || *value == b'_')
    };
    (0..bytes.len().saturating_sub(name.len() - 1)).find(|start| {
        bytes[*start..].starts_with(name)
            && !word(start.checked_sub(1).and_then(|index| bytes.get(index)))
            && !word(bytes.get(start + name.len()))
    })
}
