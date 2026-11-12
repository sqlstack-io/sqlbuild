//! Columns and kinds of the display-only observations state table.

use crate::freshness::constants::{STRING_STATE_VALUE_TYPE, TEXT_TIMESTAMP_STATE_VALUE_TYPE};

/// The column the warehouse clock fills on insert.
pub const OBSERVED_AT_COLUMN: &str = "observed_at";
/// Columns in declaration order with their state value types; `observed_at` is last.
pub const OBSERVATION_COLUMN_TYPES: [(&str, &str); 7] = [
    ("observation_id", STRING_STATE_VALUE_TYPE),
    ("kind", STRING_STATE_VALUE_TYPE),
    ("node", STRING_STATE_VALUE_TYPE),
    ("cursor_type", STRING_STATE_VALUE_TYPE),
    ("value", STRING_STATE_VALUE_TYPE),
    ("run_id", STRING_STATE_VALUE_TYPE),
    (OBSERVED_AT_COLUMN, TEXT_TIMESTAMP_STATE_VALUE_TYPE),
];
/// Columns that may be NULL: only `cursor_max` observations have a cursor type.
pub const NULLABLE_OBSERVATION_COLUMNS: [&str; 1] = ["cursor_type"];
/// Observation of a model's cursor `MAX` after a successful run.
pub const CURSOR_MAX_KIND: &str = "cursor_max";
/// Observation of the columns a live `DESCRIBE` returned for an expression source.
pub const SOURCE_COLUMNS_KIND: &str = "source_columns";
