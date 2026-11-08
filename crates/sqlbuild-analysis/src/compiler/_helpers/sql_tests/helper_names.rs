//! Reserved top-level names for test-defined helper CTEs and the rewrite of their references.

use std::collections::{HashMap, HashSet};

use sqlquack_syntax::{Expression, ExpressionWalk, Parsed};

use crate::analysis_route::types::NativeDialect;

pub(crate) const HELPER_PREFIX: &str = "__helper__";

/// The reserved top-level CTE name of one test-defined helper.
pub(crate) fn helper_cte_name(name: &str) -> String {
    format!("{HELPER_PREFIX}{name}")
}

/// Point unqualified relation references at reserved helper names by AST span, keeping their alias.
pub(crate) fn rename_helper_references(
    sql: &str,
    helpers: &HashMap<String, String>,
    parser: &dyn NativeDialect,
) -> Option<String> {
    let statements = match parser.parse(sql) {
        Ok(Parsed { statements, gaps }) if gaps.is_empty() => statements,
        _ => return None,
    };
    let [statement] = statements.as_slice() else {
        return None;
    };
    let char_offsets: Vec<usize> = sql
        .char_indices()
        .map(|(offset, _)| offset)
        .chain(std::iter::once(sql.len()))
        .collect();
    let mut aliased: HashSet<usize> = HashSet::new();
    for node in statement.dfs() {
        if let Expression::Alias(alias) = node
            && let Expression::Table(table) = &alias.this
            && let Some(span) = table.name.span
        {
            aliased.insert(span.start);
        }
    }
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    for node in statement.dfs() {
        let Expression::Table(table) = node else {
            continue;
        };
        if table.schema.is_some() || table.catalog.is_some() {
            continue;
        }
        let Some(reserved) = helpers.get(&table.name.name.to_lowercase()) else {
            continue;
        };
        let span = table.name.span?;
        let start = *char_offsets.get(span.start)?;
        let end = *char_offsets.get(span.end)?;
        let authored = sql.get(start..end)?;
        let replacement = if table.alias.is_some() || aliased.contains(&span.start) {
            reserved.clone()
        } else {
            format!("{reserved} AS {authored}")
        };
        edits.push((start, end, replacement));
    }
    edits.sort_by_key(|(start, _, _)| *start);
    let mut rewritten = String::with_capacity(sql.len() + edits.len() * HELPER_PREFIX.len());
    let mut cursor = 0;
    for (start, end, replacement) in edits {
        if start < cursor {
            return None;
        }
        rewritten.push_str(&sql[cursor..start]);
        rewritten.push_str(&replacement);
        cursor = end;
    }
    rewritten.push_str(&sql[cursor..]);
    Some(rewritten)
}
