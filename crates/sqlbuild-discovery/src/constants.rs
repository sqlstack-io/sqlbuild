//! Project layout names shared by native discovery.

/// Root of SQL model files.
pub const MODELS_DIRECTORY: &str = "models";
/// Root of code-location orchestration declarations, never imported by the scheduler.
pub const ORCHESTRATION_DIRECTORY: &str = "orchestration";
/// Suffix of SQL files.
pub const SQL_FILE_SUFFIX: &str = ".sql";
/// The folder below a resource tree that holds grouped scoped declarations.
pub const DECLARATION_GROUP_DIRECTORY: &str = "_sqlbuild";
/// Inherited and local declaration directories whose files are never resources.
pub const SCOPED_DECLARATION_DIRECTORIES: [&str; 6] = [
    "macros",
    "enums",
    "constants",
    "_macros",
    "_enums",
    "_constants",
];
/// Resource trees that may hold scoped declarations, in Python's lookup order.
pub const CANONICAL_AUTHORED_ROOTS: [&[&str]; 5] = [
    &["models"],
    &["tests", "unit"],
    &["tests", "scenarios"],
    &["functions", "sql"],
    &["sources"],
];
/// The SQL test root, whose `macros/` SQL files are tests rather than declarations.
pub const SQL_TESTS_ROOT: &[&str] = &["tests", "unit"];
/// The SQL scenario root.
pub const SQL_SCENARIOS_ROOT: &str = "tests/scenarios";
/// The directory of macro tests below the SQL test root.
pub const MACRO_TESTS_DIRECTORY: &str = "macros";
/// Inherited declaration directories, which may also be project-wide global roots.
pub const GLOBAL_DECLARATION_DIRECTORIES: [&str; 3] = ["macros", "enums", "constants"];
/// Folder-local declaration directories.
pub const LOCAL_DECLARATION_DIRECTORIES: [&str; 3] = ["_macros", "_enums", "_constants"];
/// Directories a declaration group may hold for named declarations.
pub const GROUPED_NAMED_DECLARATION_DIRECTORIES: [&str; 6] = [
    "audits", "_audits", "schemas", "_schemas", "hooks", "_hooks",
];
/// Project-wide named declaration roles.
pub const GLOBAL_NAMED_DECLARATION_DIRECTORIES: [&str; 3] = ["audits", "schemas", "hooks"];
/// Suffix of Python macro files.
pub const PYTHON_FILE_SUFFIX: &str = ".py";
/// The folder of a macro role that holds Python macros.
pub const PYTHON_MACROS_DIRECTORY: &str = "python";
/// The folder of a macro role that holds SQL macros.
pub const SQL_MACROS_DIRECTORY: &str = "sql";
/// The project-wide audit role directory.
pub const AUDIT_ROLE_DIRECTORY: &str = "audits";
/// The folder-local audit role directory inside a declaration group.
pub const LOCAL_AUDIT_ROLE_DIRECTORY: &str = "_audits";
/// The singular audit directory below an audit role.
pub const SINGULAR_AUDIT_DIRECTORY: &str = "singular";
/// Header keys a `MODEL` statement accepts, sorted by code point.
pub const SQL_MODEL_HEADER_KEYS: [&str; 74] = [
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
    "critical",
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
/// `MODEL` header keys that were removed and fail with a dedicated message.
pub const REMOVED_SQL_MODEL_HEADER_KEYS: [&str; 1] = ["run_despite_unchanged"];
/// Header keys a `FUNCTION` statement accepts.
pub const SQL_FUNCTION_HEADER_KEYS: [&str; 6] = [
    "arguments",
    "database",
    "description",
    "returns",
    "schema",
    "tags",
];
/// Header keys an `AUDIT` statement accepts.
pub const SQL_AUDIT_HEADER_KEYS: [&str; 11] = [
    "always_run",
    "evaluation",
    "minimum_samples",
    "name",
    "run_scope",
    "sample_count",
    "sample_unit",
    "severity",
    "sql_analysis",
    "thresholds",
    "value",
];
/// Header keys a `HOOK` statement accepts.
pub const SQL_HOOK_HEADER_KEYS: [&str; 1] = ["description"];
/// Header keys a `TEST` statement accepts.
pub const SQL_TEST_HEADER_KEYS: [&str; 8] = [
    "cases",
    "cursor_end",
    "cursor_start",
    "mode",
    "name",
    "parameters",
    "sql_analysis",
    "timeout",
];
/// Header keys a `SCENARIO` statement accepts.
pub const SQL_SCENARIO_HEADER_KEYS: [&str; 4] =
    ["description", "expected_columns", "tags", "timeout"];
/// `SCENARIO (expected_columns ...)` values: compare listed columns, or require every column.
pub const SCENARIO_EXPECTED_COLUMNS_KEY: &str = "expected_columns";
pub const SCENARIO_EXPECTED_COLUMNS_VALUES: [&str; 2] = ["listed", "all"];
pub const SCENARIO_EXPECTED_COLUMNS_ALL: &str = "all";
