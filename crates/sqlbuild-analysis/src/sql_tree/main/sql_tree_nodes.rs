//! `sql_tree_nodes`: a rule's SQL syntax tree as nodes in pre-order.

use sqlbuild_sqltext::sql_scan::main::dialect_lexical_syntax::dialect_lexical_syntax;
use sqlquack_syntax::Expression;

use crate::relation_markers::main::located_markers_as_names::located_markers_as_names;
use crate::sql_tree::_helpers::parse::parse;
use crate::sql_tree::_helpers::rules_tree_parser::rules_tree_parser;
use crate::sql_tree::_helpers::source_map::SourceMap;
use crate::sql_tree::_helpers::table_wrapped::table_wrapped;
use crate::sql_tree::_helpers::walk::walk;
use crate::sql_tree::constants::{SNOWFLAKE_DIALECT, TREE_WORKER_STACK_BYTES};
use crate::sql_tree::errors::SqlTreeError;
use crate::sql_tree::models::SqlTreeNode;
use crate::worker_stack::main::run_on_worker_stack::run_on_worker_stack;

/// Every node of `sql` parsed in `dialect`, in pre-order, joins and CTEs owning their expressions; relation markers read as tables named by their calls, and every position is in `sql`.
pub fn sql_tree_nodes(sql: &str, dialect: &str) -> Result<Vec<SqlTreeNode>, SqlTreeError> {
    run_on_worker_stack(TREE_WORKER_STACK_BYTES, "the parser panicked", || {
        tree_nodes(sql, dialect)
    })
    .map_err(SqlTreeError::Worker)?
}

fn tree_nodes(sql: &str, dialect: &str) -> Result<Vec<SqlTreeNode>, SqlTreeError> {
    let parser = rules_tree_parser(dialect)
        .ok_or_else(|| SqlTreeError::UnavailableDialect(dialect.to_owned()))?;
    let syntax = dialect_lexical_syntax(dialect);
    let (named, calls, ranges) =
        located_markers_as_names(sql, &syntax).map_err(SqlTreeError::Unanalysable)?;
    let (wrapped, wrappers) = if dialect == SNOWFLAKE_DIALECT {
        let (wrapped, wrappers) = table_wrapped(&named, &calls, &syntax);
        (Some(wrapped), wrappers)
    } else {
        (None, Vec::new())
    };
    let map = SourceMap::new(sql, &named, &ranges, wrappers);
    let statements: Vec<Expression> = parse(wrapped.as_deref().unwrap_or(&named), parser)?;
    walk(&statements, &map)
}
