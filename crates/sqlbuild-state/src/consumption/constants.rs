//! Columns, phases, input kinds and read bounds of the consumption state table.

use crate::freshness::constants::{STRING_STATE_VALUE_TYPE, TEXT_TIMESTAMP_STATE_VALUE_TYPE};

/// State value type of an integer column.
pub const INTEGER_STATE_VALUE_TYPE: &str = "integer";
/// The column the warehouse clock fills on insert.
pub const RECORDED_AT_COLUMN: &str = "recorded_at";
/// Columns in declaration order with their state value types; `recorded_at` is last.
pub const CONSUMPTION_COLUMN_TYPES: [(&str, &str); 14] = [
    ("execution_id", STRING_STATE_VALUE_TYPE),
    ("phase", STRING_STATE_VALUE_TYPE),
    ("run_id", STRING_STATE_VALUE_TYPE),
    ("node_type", STRING_STATE_VALUE_TYPE),
    ("node_name", STRING_STATE_VALUE_TYPE),
    ("target_database", STRING_STATE_VALUE_TYPE),
    ("target_schema", STRING_STATE_VALUE_TYPE),
    ("target_name", STRING_STATE_VALUE_TYPE),
    ("version_hash", STRING_STATE_VALUE_TYPE),
    ("input_count", INTEGER_STATE_VALUE_TYPE),
    ("input_kind", STRING_STATE_VALUE_TYPE),
    ("input_name", STRING_STATE_VALUE_TYPE),
    ("input_version", STRING_STATE_VALUE_TYPE),
    (RECORDED_AT_COLUMN, TEXT_TIMESTAMP_STATE_VALUE_TYPE),
];
/// Columns that may be NULL: the relation's database and the input of `started` and input-less rows.
pub const NULLABLE_COLUMNS: [&str; 4] = [
    "target_database",
    "input_kind",
    "input_name",
    "input_version",
];
/// Phase of the row appended before a model or seed writes.
pub const STARTED_PHASE: &str = "started";
/// Phase of the rows appended after the write succeeded.
pub const SUCCEEDED_PHASE: &str = "succeeded";
/// Input kind of a source, versioned by its observed `data_version_hash`.
pub const SOURCE_INPUT: &str = "source";
/// Input kind of an upstream model or seed, versioned by its `execution_id`.
pub const NODE_INPUT: &str = "node";
/// Input kind of anything whose version cannot be known.
pub const OPAQUE_INPUT: &str = "opaque";
/// Planner change kinds that are edits of the node's own definition.
pub const DEFINITION_PLANNER_CHANGES: [&str; 3] =
    ["query_changed", "function_changed", "config_changed"];
/// The version recorded for an input whose current version is unknown.
pub const UNKNOWN_VERSION: &str = "unknown";
/// Executions read per relation, newest first.
pub const RECENT_EXECUTIONS: usize = 8;
/// Executions of one relation appended this close to the newest are all checked, in microseconds.
pub const ORDER_SKEW_MICROS: i64 = 5_000_000;
