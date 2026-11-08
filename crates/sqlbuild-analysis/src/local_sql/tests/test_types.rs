//! Test case types for local scenario SQL conversion.

/// SQL, the result columns it converts with (none when refused), and text its SQL or refusal holds.
pub(crate) struct LocalSqlTestCase {
    pub(crate) description: &'static str,
    pub(crate) sql: &'static str,
    pub(crate) expected_columns: Vec<(String, String)>,
    pub(crate) expected_fragments: Vec<&'static str>,
}
