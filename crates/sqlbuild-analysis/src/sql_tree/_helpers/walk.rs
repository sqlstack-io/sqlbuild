//! The iterative pre-order walk that turns parsed statements into rule tree nodes.

use std::collections::HashMap;

use sqlquack_syntax::tokens::models::Span;
use sqlquack_syntax::{Expression, ExpressionWalk};

use crate::sql_tree::_helpers::clauses::clauses;
use crate::sql_tree::_helpers::positions::line_column;
use crate::sql_tree::constants::OPAQUE_KINDS;
use crate::sql_tree::errors::SqlTreeError;
use crate::sql_tree::models::SqlTreeNode;

/// Child expressions mapped to the index of the clause node that owns them.
type Owners = HashMap<*const Expression, usize>;

/// The nodes of `statements` in pre-order, walked with an explicit stack so depth cannot overflow.
pub(crate) fn walk(
    statements: &[Expression],
    starts: &[usize],
) -> Result<Vec<SqlTreeNode>, SqlTreeError> {
    let mut nodes: Vec<SqlTreeNode> = Vec::new();
    let mut stack: Vec<(&Expression, Option<usize>)> = statements
        .iter()
        .rev()
        .map(|statement| (statement, None))
        .collect();
    while let Some((expression, parent)) = stack.pop() {
        let index: usize = nodes.len();
        let node: SqlTreeNode = expression_node(&nodes, expression, parent, starts)?;
        let (clause_nodes, owners) = clause_nodes(expression, &node, index, starts);
        nodes.push(node);
        nodes.extend(clause_nodes);
        for child in expression.children().into_iter().rev() {
            let owner: usize = owners
                .get(&std::ptr::from_ref(child))
                .copied()
                .unwrap_or(index);
            stack.push((child, Some(owner)));
        }
    }
    Ok(nodes)
}

/// `expression`'s node, at its own start or else at its parent's.
fn expression_node(
    nodes: &[SqlTreeNode],
    expression: &Expression,
    parent: Option<usize>,
    starts: &[usize],
) -> Result<SqlTreeNode, SqlTreeError> {
    let kind: &'static str = expression.variant_name();
    if OPAQUE_KINDS.contains(&kind) {
        return Err(SqlTreeError::Unanalysable(
            "a statement uses syntax the parser keeps as unparsed text".to_owned(),
        ));
    }
    let inherited = parent.map_or((1, 1), |parent| (nodes[parent].line, nodes[parent].column));
    let (line, column) =
        own_span(expression).map_or(inherited, |span| line_column(starts, span.start));
    Ok(SqlTreeNode {
        kind,
        line,
        column,
        parent,
    })
}

/// The clause nodes `expression` holds, numbered after `index`, and the children each one owns.
fn clause_nodes(
    expression: &Expression,
    node: &SqlTreeNode,
    index: usize,
    starts: &[usize],
) -> (Vec<SqlTreeNode>, Owners) {
    let mut built: Vec<SqlTreeNode> = Vec::new();
    let mut owners: Owners = HashMap::new();
    for clause in clauses(expression) {
        let clause_index: usize = index + 1 + built.len();
        let (line, column) = clause.span.map_or((node.line, node.column), |span| {
            line_column(starts, span.start)
        });
        built.push(SqlTreeNode {
            kind: clause.kind,
            line,
            column,
            parent: Some(index),
        });
        for child in clause.children {
            let _ = owners.insert(std::ptr::from_ref(child), clause_index);
        }
    }
    (built, owners)
}

/// The node's span; a qualified star without one starts at its qualifier.
fn own_span(expression: &Expression) -> Option<Span> {
    expression.source_span().or_else(|| match expression {
        Expression::Star(star) => star.table.as_ref().and_then(|table| table.span),
        _ => None,
    })
}
