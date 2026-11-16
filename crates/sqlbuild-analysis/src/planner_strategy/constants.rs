//! Planner strategy constants shared with the Python planner.

use crate::planner_strategy::models::PlanAction;

/// Error code of an incremental model without a strategy.
pub const MISSING_STRATEGY_ERROR_CODE: &str = "S201";
/// Error code of an incremental model with an unknown strategy.
pub const UNKNOWN_STRATEGY_ERROR_CODE: &str = "S202";
/// Error code of a watermark input outside the model's lineage.
pub const LINEAGE_INPUT_ERROR_CODE: &str = "S302";
/// Warning code of a recorded relation that no longer exists.
pub const RECORDED_RELATION_MISSING_WARNING_CODE: &str = "RECORDED_RELATION_MISSING";
/// Warning code of a model planning more batches than its limit.
pub const MICROBATCH_LIMIT_WARNING_CODE: &str = "MICROBATCH_LIMIT_EXCEEDED";
/// The `on_schema_change` value of a model that leaves schema changes alone.
pub const IGNORE_SCHEMA_CHANGE: &str = "ignore";
/// The `on_schema_change` value of a model that fails on schema changes.
pub const FAIL_SCHEMA_CHANGE: &str = "fail";
/// The `on_schema_change` value that adds new columns; the default.
pub const APPEND_NEW_COLUMNS: &str = "append_new_columns";
/// The `on_schema_change` value that adds, drops and retypes columns.
pub const SYNC_ALL_COLUMNS: &str = "sync_all_columns";
/// Incremental strategies and the plan action of each.
pub const INCREMENTAL_STRATEGIES: [(&str, PlanAction); 3] = [
    ("append", PlanAction::IncrementalAppend),
    ("delete_insert", PlanAction::IncrementalDeleteInsert),
    ("merge", PlanAction::IncrementalMerge),
];
/// How to silence forward-only change warnings.
pub const QUERY_CHANGE_TRACKING_HELP: &str = "To silence these warnings, set this in sqlbuild_project.toml:\n            [settings]\n            query_change_tracking = false";
/// Reference kind of a model reference.
pub const MODEL_REFERENCE_KIND: &str = "ref";
/// Reference kind of a source reference.
pub const SOURCE_REFERENCE_KIND: &str = "source";
/// Reference kinds of SQL function calls.
pub const FUNCTION_REFERENCE_KINDS: [&str; 2] = ["udf", "table_fn"];
/// Warehouse type names classified as timestamp cursors.
pub const TIMESTAMP_TYPE_NAMES: [&str; 13] = [
    "TIMESTAMP",
    "TIMESTAMPNTZ",
    "TIMESTAMPLTZ",
    "TIMESTAMPTZ",
    "TIMESTAMP_S",
    "TIMESTAMP_MS",
    "TIMESTAMP_NS",
    "DATE",
    "DATE32",
    "DATETIME",
    "DATETIME2",
    "DATETIME64",
    "SMALLDATETIME",
];
/// Warehouse type names classified as integer cursors.
pub const INTEGER_TYPE_NAMES: [&str; 17] = [
    "INT",
    "BIG_INT",
    "BIGINT",
    "SMALL_INT",
    "SMALLINT",
    "TINY_INT",
    "TINYINT",
    "MEDIUMINT",
    "INT128",
    "INT256",
    "UINT",
    "UBIGINT",
    "USMALLINT",
    "UTINYINT",
    "UMEDIUMINT",
    "UINT128",
    "UINT256",
];
/// Decimal type names whose zero-scale form is an integer cursor.
pub const DECIMAL_TYPE_NAMES: [&str; 3] = ["DECIMAL", "NUMERIC", "NUMBER"];
/// The type Polyglot reports for a type it does not know.
pub const POLYGLOT_CUSTOM_TYPE_NAME: &str = "CUSTOM";
/// The dialect cursor type classification parses unknown types with.
pub const GENERIC_DIALECT: &str = "generic";
/// Error code of an authored cursor input column its input does not expose.
pub const CURSOR_INPUT_COLUMN_ERROR_CODE: &str = "S302";
/// Incremental strategies whose microbatches replay one batch by default.
pub const IDEMPOTENT_MICROBATCH_STRATEGIES: [&str; 2] = ["delete_insert", "merge"];
/// The table type that drops the adapter's transient DDL keyword.
pub const PERMANENT_TABLE_TYPE: &str = "permanent";
/// The transient table DDL a permanent table rewrites once.
pub const TRANSIENT_TABLE_DDL: &str = "TRANSIENT TABLE";
/// The permanent table DDL replacing the transient keyword.
pub const PERMANENT_TABLE_DDL: &str = "TABLE";
/// The warning code of an incremental model whose table keeps an outdated physical layout.
pub const PHYSICAL_LAYOUT_PENDING_WARNING_CODE: &str = "PHYSICAL_LAYOUT_PENDING";
/// Model config keys of a table's physical layout, in the order warnings name them.
pub const PHYSICAL_LAYOUT_CONFIG_KEYS: [&str; 5] = [
    "engine",
    "order_by",
    "partition_by",
    "primary_key",
    "table_settings",
];
/// The version metadata member holding the model config.
pub const METADATA_CONFIG_MEMBER: &str = "config";
