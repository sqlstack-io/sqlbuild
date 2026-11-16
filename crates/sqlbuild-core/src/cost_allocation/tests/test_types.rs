use crate::cost_allocation::models::PyDecimal;

pub(crate) struct DecimalOperationTestCase {
    pub(crate) description: &'static str,
    pub(crate) result: PyDecimal,
    pub(crate) expected_text: &'static str,
}

pub(crate) struct AllocationTestCase {
    pub(crate) description: &'static str,
    pub(crate) concurrent_queries: u64,
    pub(crate) expected_seconds: &'static str,
    pub(crate) expected_usd: &'static str,
}

pub(crate) struct StatementTimingsSqlTestCase {
    pub(crate) description: &'static str,
    pub(crate) database: Option<&'static str>,
    pub(crate) query_ids: Vec<&'static str>,
    pub(crate) expected_sql: &'static str,
}
