//! Test case types for relation markers read as names.

/// SQL in one dialect, the SQL its markers read as, and the calls they stand for.
pub(crate) struct MarkersAsNamesTestCase {
    pub(crate) description: &'static str,
    pub(crate) dialect: &'static str,
    pub(crate) sql: &'static str,
    pub(crate) expected_sql: String,
    pub(crate) expected_calls: &'static [(&'static str, &'static str)],
}
