//! A persisted cost run as plain data; decimals and timestamps keep Python's text form.

/// One resource row of a run's cost (`ResourceCost`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CostResource {
    pub resource_type: String,
    pub resource_name: String,
    pub warehouse_name: String,
    pub warehouse_size: String,
    pub warehouse_type: Option<String>,
    pub cluster_number: Option<i64>,
    pub query_count: i64,
    /// `str(Decimal)` text.
    pub attributed_seconds: String,
    pub bytes_scanned: i64,
    pub estimated_compute_credits: String,
    pub estimated_usd: String,
}

/// A run's cost (`RunCostSummary`); decimal fields are `str(Decimal)` text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CostSummary {
    pub status: String,
    pub usd_per_credit: String,
    pub rate_source: String,
    pub resources: Vec<CostResource>,
    pub query_count: i64,
    pub attributed_seconds: String,
    pub bytes_scanned: i64,
    pub estimated_compute_credits: String,
    pub estimated_usd: String,
    pub expected_statement_count: i64,
    pub observed_statement_count: i64,
    pub missing_statement_count: i64,
    pub skipped_statement_count: i64,
    pub source: String,
    pub method: String,
    pub limitations: Vec<String>,
    pub message: Option<String>,
}

/// One persisted run (`CostRunRecord`); timestamps are `datetime.isoformat()` text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CostRun {
    pub run_id: String,
    pub adapter_name: String,
    pub target_name: Option<String>,
    pub build_status: String,
    pub started_at: String,
    pub completed_at: String,
    pub cost: CostSummary,
    pub version: i64,
    pub had_executable_work: Option<bool>,
}

/// How a run's breakdown is laid out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BreakdownOptions {
    pub use_color: bool,
    /// Show at most this many resources.
    pub limit: Option<usize>,
    pub terminal_width: usize,
    pub show_run_id: bool,
}

/// Canonical exact-decimal order, without expanding scientific exponents.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum DecimalSortKey {
    Negative(std::cmp::Reverse<(i128, String)>),
    Zero,
    Positive((i128, String)),
}
