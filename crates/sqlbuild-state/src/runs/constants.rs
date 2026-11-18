//! Columns, phases and statuses of the append-only runs state table.

use crate::consumption::constants::INTEGER_STATE_VALUE_TYPE;
use crate::freshness::constants::{STRING_STATE_VALUE_TYPE, TEXT_TIMESTAMP_STATE_VALUE_TYPE};

/// The column the warehouse clock fills on insert.
pub const RECORDED_AT_COLUMN: &str = "recorded_at";
/// Columns in declaration order with their state value types; `recorded_at` is last.
pub const RUN_COLUMN_TYPES: [(&str, &str); 21] = [
    ("run_id", STRING_STATE_VALUE_TYPE),
    ("phase", STRING_STATE_VALUE_TYPE),
    ("attempt", INTEGER_STATE_VALUE_TYPE),
    ("retry_of_run_id", STRING_STATE_VALUE_TYPE),
    ("orchestration_run_id", STRING_STATE_VALUE_TYPE),
    ("command", STRING_STATE_VALUE_TYPE),
    ("select_json", STRING_STATE_VALUE_TYPE),
    ("exclude_json", STRING_STATE_VALUE_TYPE),
    ("named_jobs_json", STRING_STATE_VALUE_TYPE),
    ("target_name", STRING_STATE_VALUE_TYPE),
    ("warehouse_user", STRING_STATE_VALUE_TYPE),
    ("sqlbuild_user", STRING_STATE_VALUE_TYPE),
    ("os_user", STRING_STATE_VALUE_TYPE),
    ("host", STRING_STATE_VALUE_TYPE),
    ("sqlbuild_version", STRING_STATE_VALUE_TYPE),
    ("git_commit", STRING_STATE_VALUE_TYPE),
    ("git_dirty", INTEGER_STATE_VALUE_TYPE),
    ("var_keys_json", STRING_STATE_VALUE_TYPE),
    ("status", STRING_STATE_VALUE_TYPE),
    ("node_counts_json", STRING_STATE_VALUE_TYPE),
    (RECORDED_AT_COLUMN, TEXT_TIMESTAMP_STATE_VALUE_TYPE),
];
/// Columns that may be NULL: links, identities and facts not known or recorded, and finished-only columns.
pub const NULLABLE_RUN_COLUMNS: [&str; 11] = [
    "retry_of_run_id",
    "orchestration_run_id",
    "target_name",
    "warehouse_user",
    "sqlbuild_user",
    "os_user",
    "host",
    "git_commit",
    "git_dirty",
    "status",
    "node_counts_json",
];
/// Phase of the row written before the first node runs.
pub const STARTED_PHASE: &str = "started";
/// Phase of the row written once the run ends, however it ends.
pub const FINISHED_PHASE: &str = "finished";
/// A finished run whose nodes all succeeded, warnings included.
pub const SUCCEEDED_STATUS: &str = "succeeded";
/// A finished run in which nothing succeeded and something failed or was skipped.
pub const FAILED_STATUS: &str = "failed";
/// A finished run stopped by an interrupt or a cancel.
pub const CANCELLED_STATUS: &str = "cancelled";
/// A finished run in which some nodes succeeded and others failed or were skipped.
pub const PARTIAL_STATUS: &str = "partial";
/// `sqlbuild_user` of a run the scheduler started.
pub const SYSTEM_SCHEDULE_USER: &str = "system:schedule";
/// `sqlbuild_user` of a run a sensor started.
pub const SYSTEM_SENSOR_USER: &str = "system:sensor";
/// `sqlbuild_user` of an automatic retry of a failed run.
pub const SYSTEM_RETRY_USER: &str = "system:retry";
/// The attempt number of a run that is not a retry.
pub const FIRST_ATTEMPT: i64 = 1;
/// `command` of a run of `sqb build`.
pub const BUILD_COMMAND: &str = "build";
/// `command` of a run of `sqb seed`.
pub const SEED_COMMAND: &str = "seed";
