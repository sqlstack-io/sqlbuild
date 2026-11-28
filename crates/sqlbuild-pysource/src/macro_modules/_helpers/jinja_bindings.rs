//! The module-level names bound to `sqlbuild.dbt.jinja` and nothing else.

use ruff_python_ast::{ExceptHandler, Expr, ExprContext, ModModule, Pattern, Stmt};
use std::collections::{HashMap, HashSet};

use crate::cpython_tree::main::child_nodes::child_nodes;
use crate::cpython_tree::main::walk_breadth_first::walk_breadth_first;
use crate::cpython_tree::models::CpythonNode;
use crate::macro_modules::constants::{JINJA_FUNCTION, JINJA_MODULE, MODULE_SEPARATOR};

/// The names `from sqlbuild.dbt import jinja` binds that no other module-level binding or
/// `global` declaration rebinds.
pub(crate) fn jinja_bindings(module: &ModModule) -> HashSet<String> {
    let counts: HashMap<String, usize> = module_binding_counts(module);
    let declared_global: HashSet<String> = global_names(module);
    module
        .body
        .iter()
        .filter_map(|statement| match statement {
            Stmt::ImportFrom(import)
                if import.level == 0
                    && import
                        .module
                        .as_ref()
                        .is_some_and(|name| name.as_str() == JINJA_MODULE) =>
            {
                Some(import)
            }
            _ => None,
        })
        .flat_map(|import| &import.names)
        .filter(|alias| alias.name.as_str() == JINJA_FUNCTION)
        .map(|alias| {
            alias
                .asname
                .as_ref()
                .map_or(JINJA_FUNCTION, |asname| asname.as_str())
                .to_owned()
        })
        .filter(|name| counts.get(name) == Some(&1) && !declared_global.contains(name))
        .collect()
}

/// How many module-scope bindings each name has, without entering functions, classes or lambdas.
fn module_binding_counts(module: &ModModule) -> HashMap<String, usize> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut pending: Vec<CpythonNode<'_>> = module.body.iter().map(CpythonNode::Stmt).collect();
    while let Some(node) = pending.pop() {
        for name in bound_names(&node) {
            *counts.entry(name).or_default() += 1;
        }
        if !node.opens_scope() {
            pending.extend(child_nodes(&node));
        }
    }
    counts
}

/// The names one node binds in the scope it belongs to.
fn bound_names(node: &CpythonNode<'_>) -> Vec<String> {
    let owned = |name: &str| vec![name.to_owned()];
    match node {
        CpythonNode::Stmt(Stmt::FunctionDef(function)) => owned(function.name.as_str()),
        CpythonNode::Stmt(Stmt::ClassDef(class)) => owned(class.name.as_str()),
        CpythonNode::Stmt(Stmt::Import(import)) => import
            .names
            .iter()
            .map(|alias| {
                alias.asname.as_ref().map_or_else(
                    || {
                        alias
                            .name
                            .as_str()
                            .split(MODULE_SEPARATOR)
                            .next()
                            .unwrap_or_default()
                            .to_owned()
                    },
                    |asname| asname.as_str().to_owned(),
                )
            })
            .collect(),
        CpythonNode::Stmt(Stmt::ImportFrom(import)) => import
            .names
            .iter()
            .map(|alias| {
                alias
                    .asname
                    .as_ref()
                    .map_or(alias.name.as_str(), |asname| asname.as_str())
                    .to_owned()
            })
            .collect(),
        CpythonNode::Expr(Expr::Name(name))
            if matches!(name.ctx, ExprContext::Store | ExprContext::Del) =>
        {
            owned(name.id.as_str())
        }
        CpythonNode::ExceptHandler(ExceptHandler::ExceptHandler(handler)) => handler
            .name
            .iter()
            .map(|name| name.as_str().to_owned())
            .collect(),
        CpythonNode::Pattern(Pattern::MatchAs(pattern)) => pattern
            .name
            .iter()
            .map(|name| name.as_str().to_owned())
            .collect(),
        CpythonNode::Pattern(Pattern::MatchStar(pattern)) => pattern
            .name
            .iter()
            .map(|name| name.as_str().to_owned())
            .collect(),
        CpythonNode::Pattern(Pattern::MatchMapping(pattern)) => pattern
            .rest
            .iter()
            .map(|name| name.as_str().to_owned())
            .collect(),
        _ => Vec::new(),
    }
}

/// Every name a `global` statement anywhere in the module declares.
fn global_names(module: &ModModule) -> HashSet<String> {
    walk_breadth_first(module.body.iter().map(CpythonNode::Stmt).collect())
        .into_iter()
        .filter_map(|node| match node {
            CpythonNode::Stmt(Stmt::Global(global)) => Some(global),
            _ => None,
        })
        .flat_map(|global| &global.names)
        .map(|name| name.as_str().to_owned())
        .collect()
}
