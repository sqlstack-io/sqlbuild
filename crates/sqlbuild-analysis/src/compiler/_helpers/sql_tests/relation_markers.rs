//! Relation marker calls in a serialized pre-order walk of a parsed statement's relations.

use sqlquack_syntax::expressions::queries::models::{From, Join};
use sqlquack_syntax::{Expression, ExpressionWalk};

use crate::compiler::_helpers::sql_tests::planning::DBT_REF_FUNCTION;
use crate::relation_markers::constants::MARKER_NAME_PREFIX;
use crate::relation_markers::types::MarkerCall;

/// Marker calls in node order, the relations `named` by `markers_as_names` read back as their calls.
pub(crate) fn named_marker_calls(expression: &Expression, named: &[MarkerCall]) -> Vec<MarkerCall> {
    expression.dfs().fold(Vec::new(), |calls, node| {
        node_marker_calls(calls, node, named)
    })
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
            .and_then(|index| match index.parse::<usize>() {
                Ok(index) => named.get(index).cloned(),
                Err(_) => None,
            }),
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
