//! Marker and call-site text replacement shared by SQL-test planning.

use std::collections::HashSet;

use regex::{Captures, Regex};

use crate::compiler::_helpers::sql_tests::planning::compile_error;
use crate::relation_markers::main::lateral_start::lateral_start;
use sqlbuild_sqltext::sql_scan::main::dialect_matching_paren::dialect_matching_paren;
use sqlbuild_sqltext::sql_scan::main::dialect_non_code_ranges::dialect_non_code_ranges;
use sqlbuild_sqltext::sql_scan::main::skip_whitespace::skip_whitespace;
use sqlbuild_sqltext::sql_scan::models::{LexicalSyntax, Unclosed};

pub(crate) fn replace_callable_markers<F>(
    sql: &str,
    (pattern, drop_lateral): (&Regex, bool),
    syntax: &LexicalSyntax,
    mut replacement: F,
) -> Result<(String, HashSet<String>), String>
where
    F: FnMut(&str, &str) -> Option<(String, bool)>,
{
    let mut protected = ProtectedRanges::new(syntax, sql);
    let mut output = String::with_capacity(sql.len());
    let mut reached: HashSet<String> = HashSet::new();
    let mut cursor = 0;
    for captures in pattern.captures_iter(sql) {
        let Some(full) = captures.get(0) else {
            continue;
        };
        if full.start() < cursor || protected.contains(full.start()) {
            continue;
        }
        let suffix_start = skip_whitespace(sql, full.end());
        if !sql[suffix_start..].starts_with('(') {
            continue;
        }
        let suffix_end = matching_paren_end(sql, suffix_start, syntax)?;
        let Some(name) = captures.get(1).map(|value| value.as_str()) else {
            continue;
        };
        let call_suffix = &sql[suffix_start..suffix_end];
        let Some((value, records_reach)) = replacement(name, call_suffix) else {
            continue;
        };
        let start = if drop_lateral {
            lateral_start(sql, full.start(), |index| !protected.contains(index))
                .filter(|lateral| *lateral >= cursor)
                .unwrap_or(full.start())
        } else {
            full.start()
        };
        output.push_str(&sql[cursor..start]);
        output.push_str(&value);
        cursor = suffix_end;
        if records_reach {
            reached.insert(name.to_string());
        }
    }
    output.push_str(&sql[cursor..]);
    Ok((output, reached))
}

fn matching_paren_end(sql: &str, open: usize, syntax: &LexicalSyntax) -> Result<usize, String> {
    dialect_matching_paren(sql.as_bytes(), open, syntax)
        .map(|close| close + 1)
        .map_err(|error| {
            compile_error(match error {
                Unclosed::BlockComment => "SQL function call contains an unclosed block comment",
                Unclosed::Quote => "SQL function call contains an unclosed quoted string",
                Unclosed::Parenthesis => "SQL function call contains an unclosed parenthesis",
            })
        })
}

pub(crate) fn replace_named_markers<F>(
    sql: &str,
    pattern: &Regex,
    syntax: &LexicalSyntax,
    mut replacement: F,
) -> String
where
    F: FnMut(&str) -> Option<String>,
{
    replace_marker_captures(sql, pattern, syntax, |captures| {
        replacement(captures.get(1)?.as_str())
    })
}

pub(crate) fn replace_dbt_ref_markers<F>(
    sql: &str,
    pattern: &Regex,
    syntax: &LexicalSyntax,
    mut replacement: F,
) -> String
where
    F: FnMut(&str) -> Option<String>,
{
    replace_marker_captures(sql, pattern, syntax, |captures| {
        let first = captures.get(1)?.as_str();
        let name = captures.get(2).map_or_else(
            || first.to_string(),
            |second| format!("{first}__{}", second.as_str()),
        );
        replacement(&name)
    })
}

fn replace_marker_captures<F>(
    sql: &str,
    pattern: &Regex,
    syntax: &LexicalSyntax,
    mut replacement: F,
) -> String
where
    F: FnMut(&Captures<'_>) -> Option<String>,
{
    let mut protected = ProtectedRanges::new(syntax, sql);
    let mut output = String::with_capacity(sql.len());
    let mut cursor = 0;
    for captures in pattern.captures_iter(sql) {
        let Some(full) = captures.get(0) else {
            continue;
        };
        if protected.contains(full.start()) {
            continue;
        }
        let Some(value) = replacement(&captures) else {
            continue;
        };
        output.push_str(&sql[cursor..full.start()]);
        output.push_str(&value);
        cursor = full.end();
    }
    output.push_str(&sql[cursor..]);
    output
}

pub(crate) fn marker_names(pattern: &Regex, syntax: &LexicalSyntax, sql: &str) -> Vec<String> {
    marker_names_in(pattern, &mut ProtectedRanges::new(syntax, sql))
}

/// Marker names outside the comments and quoted text of the SQL that `protected` scans.
pub(crate) fn marker_names_in(pattern: &Regex, protected: &mut ProtectedRanges<'_>) -> Vec<String> {
    let sql = protected.sql;
    let mut names: Vec<String> = Vec::new();
    for captures in pattern.captures_iter(sql) {
        let Some(full) = captures.get(0) else {
            continue;
        };
        if protected.contains(full.start()) {
            continue;
        }
        if let Some(name) = captures.get(1) {
            names.push(name.as_str().to_string());
        }
    }
    names
}

pub(crate) fn protected_ranges(syntax: &LexicalSyntax, sql: &str) -> Vec<(usize, usize)> {
    dialect_non_code_ranges(sql, syntax)
}

/// Whether `index` falls inside one of the sorted, non-overlapping `ranges`.
pub(crate) fn in_protected_range(index: usize, ranges: &[(usize, usize)]) -> bool {
    let following = ranges.partition_point(|(start, _)| *start <= index);
    following > 0 && index < ranges[following - 1].1
}

/// Comment and quoted-text ranges of one SQL text, scanned only when a marker needs them.
pub(crate) struct ProtectedRanges<'a> {
    sql: &'a str,
    syntax: &'a LexicalSyntax,
    ranges: Option<Vec<(usize, usize)>>,
}

impl<'a> ProtectedRanges<'a> {
    pub(crate) fn new(syntax: &'a LexicalSyntax, sql: &'a str) -> Self {
        Self {
            sql,
            syntax,
            ranges: None,
        }
    }

    pub(crate) fn contains(&mut self, index: usize) -> bool {
        let ranges = self
            .ranges
            .get_or_insert_with(|| protected_ranges(self.syntax, self.sql));
        in_protected_range(index, ranges)
    }
}
