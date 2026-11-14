//! A sensor's condition nodes, following names bound at module level in the same file.

use std::collections::{BTreeMap, BTreeSet};

use ruff_python_ast::{Expr, Stmt};

use crate::cpython_tree::main::walk_breadth_first::walk_breadth_first;
use crate::cpython_tree::models::CpythonNode;

/// Each module-level `NAME = value` or `NAME: T = value` with one plain name target.
pub(crate) fn module_assignments(body: &[Stmt]) -> BTreeMap<&str, &Expr> {
    body.iter().filter_map(assignment).collect()
}

/// Every node under `roots`, plus the nodes of each same-module assignment a name among them binds.
pub(crate) fn condition_nodes<'a>(
    roots: Vec<CpythonNode<'a>>,
    assignments: &BTreeMap<&'a str, &'a Expr>,
) -> Vec<CpythonNode<'a>> {
    let mut nodes: Vec<CpythonNode<'a>> = Vec::new();
    let mut followed: BTreeSet<&str> = BTreeSet::new();
    let mut pending: Vec<CpythonNode<'a>> = roots;
    while !pending.is_empty() {
        let walked: Vec<CpythonNode<'a>> = walk_breadth_first(std::mem::take(&mut pending));
        for node in &walked {
            let CpythonNode::Expr(Expr::Name(name)) = node else {
                continue;
            };
            let Some((bound, value)) = assignments.get_key_value(name.id.as_str()) else {
                continue;
            };
            if followed.insert(bound) {
                pending.push(CpythonNode::Expr(value));
            }
        }
        nodes.extend(walked);
    }
    nodes
}

fn assignment(statement: &Stmt) -> Option<(&str, &Expr)> {
    match statement {
        Stmt::Assign(assign) => match assign.targets.as_slice() {
            [Expr::Name(target)] => Some((target.id.as_str(), assign.value.as_ref())),
            _ => None,
        },
        Stmt::AnnAssign(assign) => match (assign.target.as_ref(), assign.value.as_deref()) {
            (Expr::Name(target), Some(value)) => Some((target.id.as_str(), value)),
            _ => None,
        },
        _ => None,
    }
}
