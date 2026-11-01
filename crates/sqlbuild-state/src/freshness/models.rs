//! Observed data versions, comparable state records and age statuses.

use sqlbuild_cursor::algebra::models::LocalDateTime;

/// Stable identity fields shared by warehouse writers and legacy history ingestion.
#[derive(Debug)]
pub struct FreshnessEventIdentity<'a> {
    pub source_name: &'a str,
    pub target_database: Option<&'a str>,
    pub target_schema: Option<&'a str>,
    pub target_name: Option<&'a str>,
    pub run_id: &'a str,
    pub strategy: &'a str,
    pub value_kind: &'a str,
    pub data_version_hash: &'a str,
    /// UTC ISO text with six fractional digits and a `+00:00` offset.
    pub observed_at: &'a str,
}

/// An observed data version of one value kind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DataVersion {
    /// A timestamp's wall time; `utc` when it was aware and is now in UTC.
    Timestamp {
        local: LocalDateTime,
        utc: bool,
    },
    /// An integer's decimal text.
    Integer(String),
    Text(String),
}

/// The fields of a stored or observed record that skip decisions compare.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComparableRecord {
    pub data_version_hash: String,
    pub value_kind: String,
    pub data_version: Option<String>,
}

/// How an observed timestamp's age compares with its policy thresholds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgeStatus {
    Pass,
    Warn,
    Error,
}

/// The shape of a single-value freshness query result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QueryShape {
    pub columns: usize,
    pub rows: usize,
    pub first_value_null: bool,
}
