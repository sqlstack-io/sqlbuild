//! Test case types for local scenario SQL conversion.

/// SQL, and the result columns it converts with or None when refused, plus text the output holds.
pub(crate) struct LocalSqlTestCase {
    pub(crate) description: &'static str,
    pub(crate) sql: &'static str,
    pub(crate) expected_columns: Option<Vec<(&'static str, &'static str)>>,
    pub(crate) expected_fragments: Vec<&'static str>,
}
