//! Test case types for rule SQL trees.

use crate::sql_tree::errors::SqlTreeError;

/// SQL, its dialect, and the `(kind, line, column, parent kind)` of every node of the listed kinds.
pub(crate) struct TreeShapeTestCase {
    pub(crate) description: &'static str,
    pub(crate) dialect: &'static str,
    pub(crate) sql: &'static str,
    pub(crate) expected_nodes: Vec<(&'static str, usize, usize, Option<&'static str>)>,
}

/// SQL that has no tree, and why.
pub(crate) struct TreeFailureTestCase {
    pub(crate) description: &'static str,
    pub(crate) dialect: &'static str,
    pub(crate) sql: String,
    pub(crate) expected_error: fn(&SqlTreeError) -> bool,
}
