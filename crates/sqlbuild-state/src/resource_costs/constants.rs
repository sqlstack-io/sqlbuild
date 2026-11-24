//! Columns, statuses and bounds of the append-only resource costs state table.

use crate::consumption::constants::INTEGER_STATE_VALUE_TYPE;
use crate::freshness::constants::{STRING_STATE_VALUE_TYPE, TEXT_TIMESTAMP_STATE_VALUE_TYPE};

/// The column the warehouse clock fills on insert.
pub const OBSERVED_AT_COLUMN: &str = "observed_at";
/// Columns in declaration order with their state value types; `observed_at` is last.
pub const RESOURCE_COST_COLUMN_TYPES: [(&str, &str); 25] = [
    ("observation_id", STRING_STATE_VALUE_TYPE),
    ("run_id", STRING_STATE_VALUE_TYPE),
    ("orchestration_run_id", STRING_STATE_VALUE_TYPE),
    ("attempt", INTEGER_STATE_VALUE_TYPE),
    ("resource_type", STRING_STATE_VALUE_TYPE),
    ("resource_name", STRING_STATE_VALUE_TYPE),
    ("warehouse_name", STRING_STATE_VALUE_TYPE),
    ("warehouse_size", STRING_STATE_VALUE_TYPE),
    ("warehouse_type", STRING_STATE_VALUE_TYPE),
    ("cluster_number", INTEGER_STATE_VALUE_TYPE),
    ("query_count", INTEGER_STATE_VALUE_TYPE),
    ("attributed_seconds", STRING_STATE_VALUE_TYPE),
    ("bytes_scanned", INTEGER_STATE_VALUE_TYPE),
    ("estimated_credits", STRING_STATE_VALUE_TYPE),
    ("estimated_usd", STRING_STATE_VALUE_TYPE),
    ("usd_per_credit", STRING_STATE_VALUE_TYPE),
    ("usd_per_credit_source", STRING_STATE_VALUE_TYPE),
    ("status", STRING_STATE_VALUE_TYPE),
    ("statements_expected", INTEGER_STATE_VALUE_TYPE),
    ("statements_observed", INTEGER_STATE_VALUE_TYPE),
    ("limitations", STRING_STATE_VALUE_TYPE),
    ("target_name", STRING_STATE_VALUE_TYPE),
    ("run_started_at", TEXT_TIMESTAMP_STATE_VALUE_TYPE),
    ("run_completed_at", TEXT_TIMESTAMP_STATE_VALUE_TYPE),
    (OBSERVED_AT_COLUMN, TEXT_TIMESTAMP_STATE_VALUE_TYPE),
];
/// Columns that may be NULL: server run, optional warehouse detail, unpriced USD and its price, and target.
pub const NULLABLE_RESOURCE_COST_COLUMNS: [&str; 7] = [
    "orchestration_run_id",
    "warehouse_type",
    "cluster_number",
    "estimated_usd",
    "usd_per_credit",
    "usd_per_credit_source",
    "target_name",
];
/// Columns added after the table's first release; opening an older table adds them.
pub const ADDED_RESOURCE_COST_COLUMNS: [&str; 2] = ["usd_per_credit", "usd_per_credit_source"];
/// Resource type of a run's totals row; executor resource types never start with an underscore.
pub const TOTAL_RESOURCE_TYPE: &str = "_total";
/// Columns a refresh reads from each run's newest run row, in this order.
pub const LATEST_RUN_COST_COLUMNS: [&str; 11] = [
    "run_id",
    "orchestration_run_id",
    "attempt",
    "status",
    "statements_expected",
    "statements_observed",
    "query_count",
    "limitations",
    "target_name",
    "run_started_at",
    "run_completed_at",
];
/// First element of every observation identity.
pub const RESOURCE_COST_TAG: &str = "resource_cost";
/// Statuses a later build re-reads while the warehouse is awake.
pub const REFRESHABLE_COST_STATUSES: [&str; 3] = ["pending", "partial", "collection_failed"];
/// Status re-read only while a statement was not visible yet or still running in query history.
pub const PARTIAL_STATUS: &str = "partial";
/// Status whose re-reads are bounded by `MAX_FAILED_COLLECTION_ATTEMPTS`.
pub const COLLECTION_FAILED_STATUS: &str = "collection_failed";
/// Collections of a run, the first included, after which a failed collection is not retried.
pub const MAX_FAILED_COLLECTION_ATTEMPTS: i64 = 3;
/// Runs older than this are not re-read: Snowflake's query history table function keeps 7 days.
pub const REFRESH_WINDOW_DAYS: i64 = 6;
/// Most runs one build re-reads, newest first.
pub const MAX_REFRESHED_RUNS: usize = 3;
/// Seconds after a run ends before a later build re-reads it, once its history has settled.
pub const REFRESH_SETTLE_SECONDS: i64 = 120;
/// Seconds in one day, for the refresh window.
pub const SECONDS_PER_DAY: i64 = 86_400;
