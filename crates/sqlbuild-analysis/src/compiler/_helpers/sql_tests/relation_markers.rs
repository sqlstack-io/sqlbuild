//! Relation marker calls in a serialized pre-order walk of a parsed statement's relations.

use sqlquack_syntax::expressions::queries::models::{From, Join};
use sqlquack_syntax::{Expression, ExpressionWalk};

use crate::compiler::_helpers::sql_tests::planning::DBT_REF_FUNCTION;

pub(crate) type MarkerCall = (String, String);

/// Return marker calls in node order, where children follow declared field order.
pub(crate) fn relation_marker_calls(expression: &Expression) -> Vec<MarkerCall> {
    expression.dfs().fold(Vec::new(), node_marker_calls)
}

/// FROM then JOIN marker calls of one node, plus the serialized `on` that traversal skips.
fn node_marker_calls(mut calls: Vec<MarkerCall>, node: &Expression) -> Vec<MarkerCall> {
    match node {
        Expression::Select(select) => calls.extend(
            select
                .from
                .iter()
                .flat_map(from_markers)
                .chain(join_markers(&select.joins)),
        ),
        Expression::From(from) => calls.extend(from_markers(from)),
        Expression::Delete(delete) => calls.extend(join_markers(&delete.joins)),
        Expression::JoinedTable(table) => {
            calls.extend(
                marker(&table.left)
                    .into_iter()
                    .chain(join_markers(&table.joins)),
            );
        }
        Expression::Update(update) => calls.extend(
            update
                .from_clause
                .iter()
                .flat_map(from_markers)
                .chain(join_markers(&update.from_joins)),
        ),
        Expression::DataDeletionProperty(property) => {
            return property.on.dfs().fold(calls, node_marker_calls);
        }
        _ => {}
    }
    calls
}

fn from_markers(from: &From) -> impl Iterator<Item = MarkerCall> + '_ {
    from.expressions.iter().filter_map(marker)
}

fn join_markers(joins: &[Join]) -> impl Iterator<Item = MarkerCall> + '_ {
    joins.iter().filter_map(|join| marker(&join.this))
}

fn marker(expression: &Expression) -> Option<MarkerCall> {
    match expression {
        Expression::Alias(alias) => marker(&alias.this),
        Expression::Function(function) => function_marker(&function.name, &function.args),
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
