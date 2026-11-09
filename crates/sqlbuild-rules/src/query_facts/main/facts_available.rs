//! `facts_available`: whether SQLBuild parses a dialect's SQL for built-in rules.

use sqlbuild_analysis::sql_tree::main::shared_dialect::shared_dialect;

/// Whether SQLBuild parses `dialect`'s SQL for the built-in rules that read query facts.
pub(crate) fn facts_available(dialect: &str) -> bool {
    shared_dialect(dialect).is_some()
}
