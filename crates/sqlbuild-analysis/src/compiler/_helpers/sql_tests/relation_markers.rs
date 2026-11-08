//! Relation marker calls in a serialized pre-order walk of a parsed statement's relations.

use std::sync::LazyLock;

use regex::Regex;
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;
use sqlquack_syntax::expressions::queries::models::{From, Join};
use sqlquack_syntax::{Expression, ExpressionWalk};

use crate::compiler::_helpers::sql_tests::markers::ProtectedRanges;
use crate::compiler::_helpers::sql_tests::planning::DBT_REF_FUNCTION;

pub(crate) type MarkerCall = (String, String);

/// The name a relation marker call stands as while the planning template is parsed.
const MARKER_NAME_PREFIX: &str = "__sqlbuild_relation_";

/// A relation marker call: `__name("x")`, or `__name("a", "b")` as `__dbt_ref` writes it.
static MARKER_CALL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)\b(__[a-z_]+)\(\s*"([^"]+)"\s*(?:,\s*"([^"]+)"\s*)?\)"#)
        .expect("the marker call pattern compiles")
});

/// Return marker calls in node order, where children follow declared field order.
pub(crate) fn relation_marker_calls(expression: &Expression) -> Vec<MarkerCall> {
    named_marker_calls(expression, &[])
}

/// Marker calls in node order, the relations `named` by `markers_as_names` read back as their calls.
pub(crate) fn named_marker_calls(expression: &Expression, named: &[MarkerCall]) -> Vec<MarkerCall> {
    expression.dfs().fold(Vec::new(), |calls, node| {
        node_marker_calls(calls, node, named)
    })
}

/// `sql` with each relation marker call outside comments and quoted text as one plain name, and
/// the calls by name; every dialect reads a name in FROM, while Snowflake refuses a bare call.
pub(crate) fn markers_as_names(sql: &str, syntax: &LexicalSyntax) -> (String, Vec<MarkerCall>) {
    let mut protected: ProtectedRanges<'_> = ProtectedRanges::new(syntax, sql);
    let mut named: String = String::with_capacity(sql.len());
    let mut calls: Vec<MarkerCall> = Vec::new();
    let mut cursor: usize = 0;
    for captures in MARKER_CALL.captures_iter(sql) {
        let (Some(full), Some(function), Some(first)) =
            (captures.get(0), captures.get(1), captures.get(2))
        else {
            continue;
        };
        if protected.contains(full.start()) {
            continue;
        }
        let function_name: String = function.as_str().to_ascii_lowercase();
        let referenced_name: String = match (captures.get(3), function_name == DBT_REF_FUNCTION) {
            (None, _) => first.as_str().to_owned(),
            (Some(second), true) => format!("{}__{}", first.as_str(), second.as_str()),
            (Some(_), false) => continue,
        };
        named.push_str(&sql[cursor..full.start()]);
        named.push_str(&format!("{MARKER_NAME_PREFIX}{}", calls.len()));
        cursor = full.end();
        calls.push((function_name, referenced_name));
    }
    named.push_str(&sql[cursor..]);
    (named, calls)
}

/// FROM then JOIN marker calls of one node, plus the serialized `on` that traversal skips.
fn node_marker_calls(
    mut calls: Vec<MarkerCall>,
    node: &Expression,
    named: &[MarkerCall],
) -> Vec<MarkerCall> {
    match node {
        Expression::Select(select) => calls.extend(
            select
                .from
                .iter()
                .flat_map(|from| from_markers(from, named))
                .chain(join_markers(&select.joins, named)),
        ),
        Expression::From(from) => calls.extend(from_markers(from, named)),
        Expression::Delete(delete) => calls.extend(join_markers(&delete.joins, named)),
        Expression::JoinedTable(table) => {
            calls.extend(
                marker(&table.left, named)
                    .into_iter()
                    .chain(join_markers(&table.joins, named)),
            );
        }
        Expression::Update(update) => calls.extend(
            update
                .from_clause
                .iter()
                .flat_map(|from| from_markers(from, named))
                .chain(join_markers(&update.from_joins, named)),
        ),
        Expression::DataDeletionProperty(property) => {
            return property
                .on
                .dfs()
                .fold(calls, |calls, node| node_marker_calls(calls, node, named));
        }
        _ => {}
    }
    calls
}

fn from_markers<'a>(
    from: &'a From,
    named: &'a [MarkerCall],
) -> impl Iterator<Item = MarkerCall> + 'a {
    from.expressions
        .iter()
        .filter_map(|expression| marker(expression, named))
}

fn join_markers<'a>(
    joins: &'a [Join],
    named: &'a [MarkerCall],
) -> impl Iterator<Item = MarkerCall> + 'a {
    joins.iter().filter_map(|join| marker(&join.this, named))
}

fn marker(expression: &Expression, named: &[MarkerCall]) -> Option<MarkerCall> {
    match expression {
        Expression::Alias(alias) => marker(&alias.this, named),
        Expression::Function(function) => function_marker(&function.name, &function.args),
        Expression::Table(table) if table.schema.is_none() => table
            .name
            .name
            .strip_prefix(MARKER_NAME_PREFIX)
            .and_then(|index| index.parse::<usize>().ok())
            .and_then(|index| named.get(index).cloned()),
        _ => None,
    }
}

fn function_marker(name: &str, args: &[Expression]) -> Option<MarkerCall> {
    let function_name = name.to_ascii_lowercase();
    let referenced_name = if function_name == DBT_REF_FUNCTION {
        match args {
            [Expression::Column(first)] => first.name.name.clone(),
            [Expression::Column(first), Expression::Column(second)] => {
                format!("{}__{}", first.name.name, second.name.name)
            }
            _ => return None,
        }
    } else {
        let [Expression::Column(first)] = args else {
            return None;
        };
        first.name.name.clone()
    };
    Some((function_name, referenced_name))
}
