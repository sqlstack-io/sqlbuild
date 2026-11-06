//! Cost summary facts a delayed refresh compares.

/// The status and statement coverage of one persisted or refreshed cost summary.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CostCoverage {
    pub status: String,
    pub expected_statement_count: i64,
    pub observed_statement_count: i64,
    pub query_count: i64,
    pub limitations: Vec<String>,
}

/// A validated cost report request; history bounds are UTC microseconds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CostReportOptions {
    pub selector: String,
    pub limit: Option<i64>,
    pub no_limit: bool,
    pub sort: String,
    pub descending: bool,
    pub since: Option<i128>,
    pub until: Option<i128>,
    pub until_exclusive: bool,
    pub use_color: bool,
    pub terminal_width: usize,
}

/// Both output forms of one report, built without Python records.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CostReport {
    pub json: String,
    pub text: String,
}

/// A cached ordering key; each request compares only one variant.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum CostSortMetric {
    Integer(i128),
    Text(String),
    Decimal(crate::cost_report::models::DecimalSortKey),
}
