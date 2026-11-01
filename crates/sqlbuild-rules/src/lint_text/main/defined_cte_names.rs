//! The CTE names a SQL text defines, as lint preparation reads them.

/// Every CTE name `sql` defines, lowercased, in order and without repeats.
#[must_use]
pub fn defined_cte_names(sql: &str) -> Vec<String> {
    crate::lint_text::_helpers::preparation::defined_cte_names(sql)
}
