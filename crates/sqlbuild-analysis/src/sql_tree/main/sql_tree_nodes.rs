//! `sql_tree_nodes`: a rule's SQL syntax tree as nodes in pre-order.

use sqlquack_syntax::Expression;

use crate::sql_tree::_helpers::parse::parse;
use crate::sql_tree::_helpers::positions::line_starts;
use crate::sql_tree::_helpers::rules_tree_parser::rules_tree_parser;
use crate::sql_tree::_helpers::walk::walk;
use crate::sql_tree::constants::TREE_WORKER_STACK_BYTES;
use crate::sql_tree::errors::SqlTreeError;
use crate::sql_tree::models::SqlTreeNode;
use crate::worker_stack::main::run_on_worker_stack::run_on_worker_stack;

/// Every node of `sql` parsed in `dialect`, in pre-order, joins and CTEs owning their expressions.
pub fn sql_tree_nodes(sql: &str, dialect: &str) -> Result<Vec<SqlTreeNode>, SqlTreeError> {
    run_on_worker_stack(TREE_WORKER_STACK_BYTES, "the parser panicked", || {
        tree_nodes(sql, dialect)
    })
    .map_err(SqlTreeError::Worker)?
}

fn tree_nodes(sql: &str, dialect: &str) -> Result<Vec<SqlTreeNode>, SqlTreeError> {
    let parser = rules_tree_parser(dialect)
        .ok_or_else(|| SqlTreeError::UnavailableDialect(dialect.to_owned()))?;
    let statements: Vec<Expression> = parse(sql, parser)?;
    walk(&statements, &line_starts(sql))
}
