//! Persisted migration identity constants.

/// Python tuple fields that the historical dict/list walkers leave opaque.
pub(crate) const OPAQUE_TUPLE_FIELDS: &[&str] = &[
    "whens",
    "rename",
    "partition",
    "partitions",
    "set",
    "credentials",
    "define",
    "fields",
    "search_results",
    "pairs",
    "path_values",
    "with_properties",
    "mysql_table_options",
    "with_options",
    "options",
    "with_params",
    "properties",
    "expressions",
    "tags",
];
pub(crate) const HAVING_MAX_KEY: &str = "having_max";
pub(crate) const WITH_KEY: &str = "with";
pub(crate) const TABLE_KEY: &str = "table";
pub(crate) const NUMBER_LITERAL: &str = "number";

/// Characters that open a quoted identifier, string or dollar-quoted body; such tokens keep their case.
pub(crate) const QUOTE_DELIMITERS: &[char] = &['\'', '"', '`', '$', '['];
pub(crate) const REF_MARKER: &str = r#"__ref\(\s*(?:"([^"]+)"|'([^']+)')\s*\)"#;
pub(crate) const EXCLUDED_CONFIG: &[&str] = &[
    "batch_size",
    "full_refresh",
    "incremental_mode",
    "incremental_strategy",
    "lookback",
    "materialized",
    "max_microbatches",
    "microbatch_limit",
    "microbatch_strategy",
    "on_schema_change",
    "snapshot_full_refresh",
    "snapshot_schema_change",
];
pub(crate) const COLUMN_ADDED: &str = "column_added";
pub(crate) const COLUMN_TYPE_CHANGED: &str = "column_type_changed";
pub(crate) const POLICY_DENY: &str = "deny";
pub(crate) const POLICY_FAIL: &str = "fail";
pub(crate) const MODEL_DONE: &str = "done";
pub(crate) const MODEL_RENAMED: &str = "renamed";
pub(crate) const MODEL_CONFLICT: &str = "conflict";
