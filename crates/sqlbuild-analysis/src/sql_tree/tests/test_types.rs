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

/// SQL, its dialect, and each node's facts as `helpers::facts` writes them.
pub(crate) struct TreeFactsTestCase {
    pub(crate) description: &'static str,
    pub(crate) dialect: &'static str,
    pub(crate) sql: &'static str,
    pub(crate) expected_facts: &'static [&'static str],
}

/// SQL, its dialect, and each clause node as `kind`, followed by its direct children as `kind line:column`, in position order.
pub(crate) struct TreeClausesTestCase {
    pub(crate) description: &'static str,
    pub(crate) dialect: &'static str,
    pub(crate) sql: &'static str,
    pub(crate) expected_clauses: Vec<(&'static str, Vec<&'static str>)>,
}

/// SQL, its dialect, and the kinds of its first statement's direct children, in tree order.
pub(crate) struct TreeChildrenTestCase {
    pub(crate) description: &'static str,
    pub(crate) dialect: &'static str,
    pub(crate) sql: &'static str,
    pub(crate) expected_children: &'static [&'static str],
}
