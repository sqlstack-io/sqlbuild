//! `markers_as_names`: relation marker calls in code as plain names that keep every position.

use regex::Regex;
use sqlbuild_sqltext::sql_scan::main::dialect_non_code_ranges::dialect_non_code_ranges;
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

use crate::relation_markers::_helpers::pattern::relation_marker_pattern;
use crate::relation_markers::constants::{DBT_REF_FUNCTION, MARKER_NAME_PREFIX};
use crate::relation_markers::types::MarkerCall;

/// `sql` with each marker call in code as `__sqb_<index>`, padded so later text keeps its line and column, and the calls by index.
pub fn markers_as_names(
    sql: &str,
    syntax: &LexicalSyntax,
) -> Result<(String, Vec<MarkerCall>), String> {
    let pattern: &Regex = relation_marker_pattern()?;
    let mut protected: Option<Vec<(usize, usize)>> = None;
    let mut named: String = String::with_capacity(sql.len());
    let mut calls: Vec<MarkerCall> = Vec::new();
    let mut cursor: usize = 0;
    for captures in pattern.captures_iter(sql) {
        let (Some(full), Some(function), Some(first)) =
            (captures.get(0), captures.get(1), captures.get(2))
        else {
            continue;
        };
        let ranges = protected.get_or_insert_with(|| dialect_non_code_ranges(sql, syntax));
        let following: usize = ranges.partition_point(|(start, _)| *start <= full.start());
        if following > 0 && full.start() < ranges[following - 1].1 {
            continue;
        }
        let function_name: String = function.as_str().to_ascii_lowercase();
        let referenced_name: String = match (captures.get(3), function_name == DBT_REF_FUNCTION) {
            (None, _) => first.as_str().to_owned(),
            (Some(second), true) => format!("{}__{}", first.as_str(), second.as_str()),
            (Some(_), false) => continue,
        };
        named.push_str(&sql[cursor..full.start()]);
        named.push_str(&padded_name(
            &format!("{MARKER_NAME_PREFIX}{}", calls.len()),
            full.as_str(),
        ));
        cursor = full.end();
        calls.push((function_name, referenced_name));
    }
    named.push_str(&sql[cursor..]);
    Ok((named, calls))
}

/// `name`, then the call's line breaks, then spaces as wide as the call's last line.
fn padded_name(name: &str, call: &str) -> String {
    let breaks: usize = call.matches('\n').count();
    let last_line: &str = call.rsplit('\n').next().unwrap_or(call);
    let width: usize = if breaks == 0 {
        call.chars().count().saturating_sub(name.chars().count())
    } else {
        last_line.chars().count()
    };
    format!("{name}{}{}", "\n".repeat(breaks), " ".repeat(width))
}
