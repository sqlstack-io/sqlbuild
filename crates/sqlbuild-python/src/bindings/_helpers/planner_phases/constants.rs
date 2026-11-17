//! Python modules and names the planner phases exchange values with.

pub(crate) const SNOWFLAKE_ADAPTER: &str = "snowflake";
pub(crate) const BIGQUERY_ADAPTER: &str = "bigquery";
pub(crate) const TRANSIENT_TABLE_TYPE: &str = "transient";
pub(crate) const PERMANENT_TABLE_TYPE: &str = "permanent";
pub(crate) const DECREASE_WARNING: &str =
    "Retention decreases are irreversible once warehouse history expires.";
pub(crate) const DOWNGRADE_HISTORY_DAYS: i64 = 90;
pub(crate) const FULL_REFRESH_DISABLED_REBUILD_CODE: &str = "S203";
pub(crate) const MISSING_UPSTREAM_CODE: &str = "S301";
pub(crate) const RETENTION_DECREASE: &str = "decrease";
pub(crate) const RETENTION_INCREASE: &str = "increase";
pub(crate) const RETENTION_MATCH: &str = "match";

/// A step's share of the plan's total time is shown as a percentage.
pub(crate) const PERCENT: f64 = 100.0;
/// Plan document timings are rounded to the millisecond.
pub(crate) const MILLISECONDS: f64 = 1000.0;

/// The planner policy, and the plan field, saying audit failures hold nothing back.
pub(crate) const IGNORE_AUDIT_FAILURES: &str = "ignore_audit_failures";
/// The plan field holding the planned audits.
pub(crate) const AUDIT_ENTRIES: &str = "audit_entries";
/// An audit plan entry's field saying its failure holds nothing back.
pub(crate) const IGNORE_FAILURE: &str = "ignore_failure";
