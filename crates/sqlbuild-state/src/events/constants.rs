//! Identity tags and messages of append-only migration state events.

pub(crate) const COLUMN_EVENT_TAG: &str = "column";
pub(crate) const OLD_NAME_EVENT_TAG: &str = "old_name";
pub(crate) const COMPACT_ITEM_SEPARATOR: &str = ",";
pub(crate) const COMPACT_KEY_SEPARATOR: &str = ":";
pub(crate) const QUOTE: char = '\'';
pub(crate) const ESCAPED_QUOTE: &str = "''";
pub(crate) const NULL_LITERAL: &str = "NULL";
pub(crate) const UTC_SUFFIX: &str = "+00:00";
pub const STRING_VALUE_REQUIRED: &str = "state string columns require str values";
pub const TIMESTAMP_VALUE_REQUIRED: &str = "state timestamp columns require datetime values";
pub(crate) const ALIAS_ITEM_SEPARATOR: &str = ",";
pub(crate) const ALIAS_KEY_SEPARATOR: &str = ":";
pub(crate) const PYTHON_TRUE: &str = "True";
pub(crate) const PYTHON_FALSE: &str = "False";
pub(crate) const PYTHON_NONE: &str = "None";
/// Event columns stored as text timestamps; every other event column is a string.
pub const TIMESTAMP_COLUMNS: [&str; 2] = ["expires_at", "created_at"];
pub const MIGRATION_COLUMNS: [&str; 15] = [
    "event_id",
    "target_name",
    "origin_model",
    "origin_database",
    "origin_schema",
    "origin_name",
    "destination_model",
    "destination_database",
    "destination_schema",
    "destination_name",
    "origin_version_hash",
    "discovery",
    "decision",
    "run_id",
    "created_at",
];
/// Columns of `_sqlbuild_node_results`, in the order the executor writes them.
pub const NODE_RESULT_COLUMNS: [&str; 12] = [
    "node_type",
    "node_name",
    "target_database",
    "target_schema",
    "target_name",
    "run_id",
    "status",
    "payload_json_b64",
    "metadata_json_b64",
    "error_message",
    "materialized",
    "ts",
];
pub const COLUMN_MIGRATION_COLUMNS: [&str; 12] = [
    "event_id",
    "target_name",
    "model_name",
    "relation_database",
    "relation_schema",
    "relation_name",
    "origin_column",
    "destination_column",
    "discovery",
    "decision",
    "run_id",
    "created_at",
];
pub const OLD_NAME_VIEW_COLUMNS: [&str; 20] = [
    "event_id",
    "target_name",
    "event_type",
    "migration_event_id",
    "destination_model",
    "old_database",
    "old_schema",
    "old_name",
    "new_database",
    "new_schema",
    "new_name",
    "view_retention",
    "archive_name",
    "column_aliases",
    "grants_copied",
    "view_sql",
    "expires_at",
    "drop_reason",
    "run_id",
    "created_at",
];
