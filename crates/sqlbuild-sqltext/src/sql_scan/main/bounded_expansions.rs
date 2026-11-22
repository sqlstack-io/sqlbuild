//! `bounded_expansions`: the one rule every textual substitution applies before it splices.

use crate::sql_scan::_helpers::boundaries::bounded;
use crate::sql_scan::models::{BoundedExpansion, LexicalSyntax};

/// The expansions over ascending byte ranges of `sql` that need a space or newline so none opens or extends a comment; usually none.
#[must_use]
pub fn bounded_expansions(
    sql: &str,
    expansions: &[(usize, usize, &str)],
    syntax: Option<&LexicalSyntax>,
) -> Vec<BoundedExpansion> {
    bounded(sql, expansions, syntax)
}
