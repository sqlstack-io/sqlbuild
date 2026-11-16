//! Python modules and names the planner phases exchange values with.

pub(crate) const SNOWFLAKE_ADAPTER: &str = "snowflake";
pub(crate) const BIGQUERY_ADAPTER: &str = "bigquery";
pub(crate) const TRANSIENT_TABLE_TYPE: &str = "transient";
pub(crate) const PERMANENT_TABLE_TYPE: &str = "permanent";
pub(crate) const DECREASE_WARNING: &str =
    "Retention decreases are irreversible once warehouse history expires.";
pub(crate) const DOWNGRADE_HISTORY_DAYS: i64 = 90;
pub(crate) const FULL_REFRESH_DISABLED_REBUILD_CODE: &str = "S203";
/// A FULL backfill of a non-microbatch model whose SQL reads the cursor intrinsics.
pub(crate) const FULL_BACKFILL_WITHOUT_INTERVAL_CODE: &str = "S204";
pub(crate) const MISSING_UPSTREAM_CODE: &str = "S301";
pub(crate) const RETENTION_DECREASE: &str = "decrease";
pub(crate) const RETENTION_INCREASE: &str = "increase";
pub(crate) const RETENTION_MATCH: &str = "match";
