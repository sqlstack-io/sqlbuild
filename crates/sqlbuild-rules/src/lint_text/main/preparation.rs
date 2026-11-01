//! Native lexical preparation with Python-compatible ASCII offsets.

use crate::lint_text::types::PreparedSql;

pub fn prepare(
    expanded: &str,
    before_expansion: &str,
    prior_sites: &[usize],
    dialect: &str,
) -> Result<Option<PreparedSql>, String> {
    crate::lint_text::_helpers::preparation::prepare(
        expanded,
        before_expansion,
        prior_sites,
        dialect,
    )
}
