//! Planner identity constants shared with the Python planner.

/// Model config keys whose values participate in version identity, in Python's insertion order after `cursor_inputs`.
pub const VERSION_IDENTITY_CONFIG_KEYS: [&str; 41] = [
    "append_cursor_inclusive",
    "batch_size",
    "check_columns",
    "cursor",
    "cursor_grain",
    "cursor_start",
    "cursor_end",
    "cursor_start_max_ahead",
    "cursor_start_max_action",
    "cursor_future_max_distance",
    "cursor_future_action",
    "cursor_type",
    "historical_input",
    "incremental_mode",
    "microbatch_strategy",
    "cursor_watermark_mode",
    "max_microbatches",
    "microbatch_limit",
    "incremental_strategy",
    "merge_exclude_columns",
    "full_refresh",
    "initial_valid_from",
    "invalidate_hard_deletes",
    "lookback",
    "materialized",
    "observed_at",
    "on_schema_change",
    "snapshot_full_refresh",
    "snapshot_schema_change",
    "snapshot_strategy",
    "unique_key",
    "replace_key",
    "updated_at",
    "valid_from_column",
    "valid_to_column",
    "engine",
    "order_by",
    "partition_by",
    "primary_key",
    "table_settings",
    "refresh_mode",
];
/// Model config key of cursor inputs, recorded before every other identity key.
pub const CURSOR_INPUTS_CONFIG_KEY: &str = "cursor_inputs";
/// Model config key of user custom config.
pub const MODEL_CUSTOM_CONFIG_KEY: &str = "config";
/// Model config key of custom placeholders.
pub const MODEL_PLACEHOLDERS_CONFIG_KEY: &str = "placeholders";
/// Model config key of pre-hooks.
pub const MODEL_PRE_HOOKS_CONFIG_KEY: &str = "pre_hooks";
/// Model config key of post-hooks.
pub const MODEL_POST_HOOKS_CONFIG_KEY: &str = "post_hooks";
/// Model config key of the contract policy.
pub const CONTRACT_CONFIG_KEY: &str = "contract";
/// The contract policy that enforces declared columns.
pub const ENFORCED_CONTRACT_POLICY: &str = "enforced";
/// Hook fields that participate in SQL hook identity.
pub const SQL_HOOK_IDENTITY_FIELDS: [&str; 4] = ["statement", "name", "definition_sql", "kwargs"];
/// Error code of a model or function that cannot be fingerprinted.
pub const QUERY_FINGERPRINT_FAILED: &str = "S025";
/// Help text of a model or function that cannot be fingerprinted.
pub const FINGERPRINT_HELP: &str = "SQLBuild fingerprints the dialect token stream of compiled SQL to detect changes. Fix the SQL so the warehouse dialect tokenizer accepts it.";
/// Resource type of models.
pub const MODEL_RESOURCE: &str = "model";
/// Resource type of seeds.
pub const SEED_RESOURCE: &str = "seed";
/// Resource type of sources.
pub const SOURCE_RESOURCE: &str = "source";
/// Resource type of scalar functions.
pub const UDF_RESOURCE: &str = "udf";
/// Resource type of table functions.
pub const TABLE_FN_RESOURCE: &str = "table_fn";
/// Resource type of SQL tests.
pub const SQL_TEST_RESOURCE: &str = "sql_test";
/// Resource type of dbt references.
pub const DBT_REF_RESOURCE: &str = "dbt_ref";
/// At most this many stale triggers are listed in one warning.
pub const STALE_TRIGGER_DISPLAY_LIMIT: usize = 5;
/// At most this many items are named in a missing-upstream message.
pub const MISSING_MESSAGE_ITEM_LIMIT: usize = 5;
