//! `rules_tree_parser`: the parser that reads a dialect's SQL for rules.

use crate::sql_tree::constants::POSTGRES_DIALECTS;
use crate::sql_tree::main::shared_dialect::shared_dialect;
use crate::sql_tree::models::TreeParser;

/// The parser for `dialect`, or `None`; add a dialect (one line) only once its parser passes the depth probe with an empty allowlist.
pub(crate) fn rules_tree_parser(dialect: &str) -> Option<TreeParser> {
    if POSTGRES_DIALECTS.contains(&dialect) {
        return Some(TreeParser::Postgres);
    }
    shared_dialect(dialect).map(TreeParser::Shared)
}
