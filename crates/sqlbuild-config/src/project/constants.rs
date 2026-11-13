//! Project configuration file names, keys and the vocabularies their values come from.

pub(crate) const LOCAL_CONFIG_FILENAME: &str = "sqlbuild_local.toml";
pub(crate) const LEGACY_PROJECT_CONFIG_FILENAME: &str = "sqlbuild_project.yml";
pub(crate) const LEGACY_LOCAL_CONFIG_FILENAME: &str = "sqlbuild_local.yml";
pub(crate) const SQL_ANALYSIS_KEY: &str = "sql_analysis";
pub(crate) const LEGACY_SQL_VALIDATION_KEY: &str = "sql_validation";
pub(crate) const CHECK_DECLARED_COLUMNS_KEY: &str = "check_declared_columns";
/// The deprecated alias of `check_declared_columns`.
pub(crate) const LEGACY_COLUMN_CONTRACT_MODE_KEY: &str = "column_contract_mode";
pub(crate) const REQUIRE_SQL_ANALYSIS_KEY: &str = "require_sql_analysis";
pub(crate) const QUERY_CHANGE_TRACKING_KEY: &str = "query_change_tracking";
pub(crate) const CONCURRENCY_KEY: &str = "concurrency";
pub(crate) const LEGACY_CONCURRENCY_KEY: &str = "max_concurrency";
pub(crate) const MAX_BATCHES_KEY: &str = "max_batches";
pub(crate) const BATCH_CONCURRENCY_KEY: &str = "batch_concurrency";
pub(crate) const MAX_MICROBATCHES_KEY: &str = "max_microbatches";
pub(crate) const OLD_NAME_VIEWS_KEY: &str = "old_name_views";
pub(crate) const USD_PER_CREDIT_KEY: &str = "usd_per_credit";
pub(crate) const RUN_NAMESPACE_KEY: &str = "run_namespace";
pub(crate) const SHUTDOWN_TIMEOUT_KEY: &str = "shutdown_timeout";
pub(crate) const LEGACY_EVENT_EXPORTERS_KEY: &str = "event_exporters";
pub(crate) const MAX_FULL_ROWS_KEY: &str = "max_full_rows";
pub(crate) const DBT_DEFER_CLONE_KEY: &str = "defer_clone_from";
pub(crate) const DBT_REUSE_KEYS: [&str; 2] = ["reuse_from", "production_ref"];
pub(crate) const DBT_REMOVED_KEYS: [&str; 2] = ["defer_clone_from", "replay_on_change"];
pub(crate) const ON_SCHEMA_CHANGE_KEY: &str = "on_schema_change";
pub(crate) const REPLAY_ON_CHANGE_KEY: &str = "replay_on_change";
pub(crate) const CHANGE_POLICY_KEYS: [&str; 2] = [ON_SCHEMA_CHANGE_KEY, REPLAY_ON_CHANGE_KEY];
pub(crate) const TEMPLATE_OPEN_TOKEN: &str = "${";
pub(crate) const MODELS_DIRECTORY_NAME: &str = "models";
pub(crate) const UNLIMITED_DIFF_FULL_ROWS: &str = "unlimited";
pub(crate) const ZERO_FIXED_DURATION: &str = "0s";
pub(crate) const DEFAULT_QUERY_ARTIFACT_TTL: &str = "24h";
pub(crate) const DEFAULT_IN_USE_WINDOW: &str = "7d";
pub(crate) const BOUNDED_REPLAY_PREFIX: &str = "bounded-";
pub(crate) const ZERO_FLOAT: f64 = 0.0;
pub(crate) const SHUTDOWN_TIMEOUT_MAX_SECONDS: u64 = 600;
pub(crate) const SUGGESTION_CUTOFF: f64 = 0.6;
pub(crate) const LISTED_SUPPORTED_KEYS_LIMIT: usize = 12;
pub(crate) const RUN_NAMESPACE_MAX_LENGTH: usize = 128;
pub(crate) const SETTING_SNIPPET_INDENT: &str = "            ";
pub(crate) const ADDITIONAL_HELP_SEPARATOR: &str = "\n  = help: ";

/// The `SettingsConfig` fields, which `[settings]` may set.
pub(crate) const UNIT_TEST_MAX_REAL_MODELS_KEY: &str = "unit_test_max_real_models";
pub(crate) const DEFAULT_UNIT_TEST_MAX_REAL_MODELS: i64 = 10;
pub(crate) const TEST_TIMEOUT_KEY: &str = "test_timeout";
pub(crate) const DELTA_COMPILE_KEY: &str = "delta_compile";
pub(crate) const SCENARIO_TIMEOUT_KEY: &str = "scenario_timeout";
pub(crate) const DEFAULT_TEST_TIMEOUT: &str = "30s";
pub(crate) const SETTINGS_KEYS: [&str; 15] = [
    "sql_analysis",
    "require_sql_analysis",
    "query_change_tracking",
    "check_declared_columns",
    "concurrency",
    "auto_load_sources",
    "microbatch_concurrency",
    "microbatch_unaccounted_partition_policy",
    "table_promotion_mode",
    "default_audit_severity",
    "default_audit_run_scope",
    "unit_test_max_real_models",
    "test_timeout",
    "scenario_timeout",
    "delta_compile",
];
pub(crate) const DEFAULTS_KEYS: [&str; 33] = [
    "append_cursor_inclusive",
    "batch_concurrency",
    "batch_size",
    "contract",
    "cursor_end",
    "cursor_future_action",
    "cursor_future_max_distance",
    "cursor_start",
    "cursor_start_max_action",
    "cursor_start_max_ahead",
    "cursor_watermark_mode",
    "database",
    "full_refresh",
    "function_database",
    "function_schema",
    "incremental_mode",
    "incremental_strategy",
    "lookback",
    "materialized",
    "max_duration",
    "max_microbatches",
    "merge_exclude_columns",
    "microbatch_strategy",
    "replay_on_change",
    "row_diff_exclude_columns",
    "row_diff_sample_rows",
    "row_diff_sample_seed",
    "row_diff_tolerances",
    "schema",
    "seed_database",
    "seed_schema",
    "tags",
    "unaccounted_partition_policy",
];
/// The MODEL header keys, which `[path_defaults]` entries may set.
pub(crate) const MODEL_HEADER_KEYS: [&str; 73] = [
    "alias",
    "append_cursor_inclusive",
    "audit_factories",
    "audits",
    "batch_concurrency",
    "batch_size",
    "check_columns",
    "columns",
    "config",
    "constants",
    "contract",
    "cursor",
    "cursor_end",
    "cursor_future_action",
    "cursor_future_max_distance",
    "cursor_grain",
    "cursor_inputs",
    "cursor_start",
    "cursor_start_max_action",
    "cursor_start_max_ahead",
    "cursor_type",
    "cursor_watermark_mode",
    "database",
    "description",
    "dynamic_columns",
    "enabled",
    "engine",
    "enums",
    "full_refresh",
    "historical_input",
    "incremental_mode",
    "incremental_strategy",
    "initial_valid_from",
    "invalidate_hard_deletes",
    "lookback",
    "materialized",
    "max_duration",
    "max_microbatches",
    "merge_exclude_columns",
    "microbatch_limit",
    "microbatch_strategy",
    "migrate_force",
    "migrate_from",
    "model_schema",
    "observed_at",
    "old_name_view",
    "on_schema_change",
    "order_by",
    "partition_by",
    "placeholders",
    "post_hooks",
    "pre_hooks",
    "primary_key",
    "replay_on_change",
    "row_diff_exclude_columns",
    "row_diff_sample_rows",
    "row_diff_sample_seed",
    "row_diff_tolerances",
    "schema",
    "snapshot_full_refresh",
    "snapshot_schema_change",
    "snapshot_strategy",
    "sql_analysis",
    "sql_validation",
    "table_settings",
    "table_type",
    "tags",
    "time_travel_retention",
    "unaccounted_partition_policy",
    "unique_key",
    "updated_at",
    "valid_from_column",
    "valid_to_column",
];
pub(crate) const TARGET_KEYS: [&str; 21] = [
    "connection",
    "catalog_connection",
    "vars",
    "database",
    "schema",
    "loader_schema",
    "defer_sources_to",
    "defer_clone_from",
    "compile_cache",
    "time_travel_retention",
    "owns_time_travel_retention_namespace",
    "default_table_type",
    "table_type_downgrade",
    "time_travel_retention_decrease",
    "missing_migration_origin",
    "clone",
    "execution_limits",
    "diff",
    "warehouses",
    "recall",
    "concurrency",
];
pub(crate) const CONTROL_KEYS: [&str; 3] = ["location", "server", "sync_at_least_every"];
/// Shortest safety sync interval, so a typo cannot bring back aggressive warehouse polling.
pub(crate) const MIN_SYNC_INTERVAL_SECONDS: u64 = 60;
pub(crate) const SYNC_INTERVAL_OFF: [&str; 3] = ["0", "0s", "off"];
pub(crate) const SYNC_INTERVAL_HELP: &str =
    "must be a duration of at least '1m' such as '30m', or '0' or 'off' to turn safety syncs off";
pub(crate) const REVIEW_KEYS: [&str; 4] =
    ["against", "critical_tags", "downstream", "schema_prefix"];
pub(crate) const REVIEW_DOWNSTREAM_POLICIES: [&str; 3] = ["critical", "all", "none"];
pub(crate) const CLONE_KEYS: [&str; 2] = ["allow_as_clone_origin", "allow_as_clone_destination"];
pub(crate) const EXECUTION_LIMIT_KEYS: [&str; 5] = [
    "max_models",
    "max_duration",
    "remediation",
    "test_timeout",
    "scenario_timeout",
];
pub(crate) const SNAPSHOT_KEYS: [&str; 4] = [
    "current_state_full_refresh",
    "historical_full_refresh",
    "schema_change",
    "wildcard_check_schema_change",
];
pub(crate) const SNAPSHOT_LIMIT_KEYS: [&str; 4] = [
    "max_rows_per_relation",
    "max_total_rows",
    "max_bytes_per_relation",
    "max_total_bytes",
];
pub(crate) const SCENARIO_KEYS: [&str; 3] =
    ["local_type_overrides", "snapshot_limits", RUN_NAMESPACE_KEY];
pub(crate) const DBT_KEYS: [&str; 5] = [
    "project_dir",
    "profiles_dir",
    "target",
    "target_path",
    "vars",
];
pub(crate) const LOCAL_DBT_KEYS: [&str; 2] = ["target", "vars"];
pub(crate) const EVENT_FILTER_KEYS: [&str; 2] = ["event_kinds", "min_severity"];
pub(crate) const EVENT_SINK_KEYS: [&str; 4] =
    ["event_kinds", "min_severity", "named", SHUTDOWN_TIMEOUT_KEY];
pub(crate) const EVENT_EXPORT_KINDS: [&str; 8] = [
    "audit",
    "invocation",
    "microbatch",
    "operation",
    "resource",
    "retry",
    "run",
    "statement",
];
/// `EVENT_EXPORT_SEVERITIES` in its declared order, which error messages list.
pub(crate) const EVENT_EXPORT_SEVERITIES: [&str; 4] = ["debug", "info", "warning", "error"];
/// `TIME_TRAVEL_RETENTION_MATERIALIZATIONS` in declared order.
pub(crate) const RETENTION_MATERIALIZATIONS: [&str; 3] = ["table", "incremental", "snapshot"];
pub(crate) const MATERIALIZATION_DEFAULT_KEYS: [&str; 2] = ["time_travel_retention", "table_type"];
pub(crate) const WAREHOUSE_GROUPS: [&str; 2] = ["build", "query"];
pub(crate) const SNAPSHOT_FULL_REFRESH_POLICIES: [&str; 3] =
    ["allow", "deny", "require_confirmation"];
pub(crate) const SNAPSHOT_SCHEMA_CHANGE_POLICIES: [&str; 3] =
    ["append_new_columns", "deny", "require_confirmation"];
pub(crate) const UNACCOUNTED_PARTITION_POLICIES: [&str; 3] =
    ["recover_all", "recover_empty", "synthesize"];
/// `column_contract_mode` values and the `check_declared_columns` value each means.
pub(crate) const COLUMN_CONTRACT_MODES: [(&str, bool); 2] =
    [("implicit", true), ("explicit", false)];
pub(crate) const AUDIT_SEVERITIES: [&str; 2] = ["warn", "error"];
pub(crate) const COLLECTION_RENDERINGS: [&str; 2] = ["value_list", "array"];
pub(crate) const FUTURE_CURSOR_ACTIONS: [&str; 2] = ["cap", "error"];
pub(crate) const MICROBATCH_LIMIT_ACTIONS: [&str; 2] = ["error", "warn"];
pub(crate) const CONTRACT_POLICIES: [&str; 2] = ["none", "enforced"];
pub(crate) const TABLE_TYPES: [&str; 2] = ["permanent", "transient"];
pub(crate) const CONFIRMATION_POLICIES: [&str; 3] = ["deny", "require_confirmation", "allow"];
pub(crate) const INHERIT_VALUE: &str = "inherit";
pub(crate) const DISABLED_VALUE: &str = "disabled";
/// `OnSchemaChange` values, sorted as the error lists them.
pub(crate) const ON_SCHEMA_CHANGE_VALUES: [&str; 4] =
    ["append_new_columns", "fail", "ignore", "sync_all_columns"];
pub(crate) const REPLAY_ON_CHANGE_VALUES: [&str; 3] = ["forward", "full", "bounded-<duration>"];
pub(crate) const ON_SCHEMA_CHANGE_EXAMPLE: &str = "append_new_columns";
pub(crate) const REPLAY_ON_CHANGE_EXAMPLE: &str = "bounded-14d";
pub(crate) const GLOB_SEGMENTS: [&str; 2] = ["*", "**"];
pub(crate) const UNSUPPORTED_GLOB_MARKERS: [char; 3] = ['*', '?', '['];
/// CPython's default `sys.get_int_max_str_digits()`, beyond which `int()` raises.
pub(crate) const PYTHON_INT_MAX_STR_DIGITS: usize = 4_300;
pub(crate) const MICROBATCH_CONCURRENCY_KEY: &str = "microbatch_concurrency";
pub(crate) const REFERENCES_SECTION: &str = "references";
pub(crate) const ENFORCE_EXPLICIT_KEY: &str = "enforce_explicit";
pub(crate) const MICROBATCH_CONCURRENCY_PURPOSE: &str = "to run microbatches concurrently";
