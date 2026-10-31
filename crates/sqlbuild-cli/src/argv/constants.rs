//! The `sqb` command table, `argparse` vocabulary and the post-parse rules' names.

use crate::argv::models::NamespaceDefault;

/// The exported `sqb` parser tree: every parser, group and action in `add_argument` order.
pub(crate) const COMMAND_TABLE: &str = include_str!("command_table.json");
/// The exported `sqb dbt plan|run|build` execution parser, its command name a placeholder.
pub(crate) const DBT_EXECUTION_TABLE: &str = include_str!("dbt_execution_table.json");
/// The placeholder of the dbt command name in the execution parser's texts.
pub(crate) const DBT_COMMAND_PLACEHOLDER: &str = "{command}";
/// `sqb dbt` execution arguments after this separator go to dbt verbatim.
pub(crate) const DBT_EXECUTION_SEPARATOR: &str = "--";
/// `argparse.SUPPRESS` as a destination.
pub(crate) const SUPPRESS_DEST: &str = "==SUPPRESS==";
/// The placeholder of the installed version in the `--version` text.
pub(crate) const VERSION_PLACEHOLDER: &str = "{version}";
pub(crate) const PREFIX_CHARS: char = '-';
pub(crate) const OPTIONS_TERMINATOR: &str = "--";
pub(crate) const USAGE_PREFIX: &str = "usage: ";
/// `SqlbuildArgumentParser.error`'s coded prefix and its `CliStyle.error_strong` colours.
pub(crate) const ERROR_PREFIX: &str = "error[C900]:";
pub(crate) const ERROR_STRONG_STYLE: &str = "\u{1b}[38;5;167m\u{1b}[1m";
pub(crate) const STYLE_RESET: &str = "\u{1b}[0m";
pub(crate) const PARSER_ERROR_EXIT: i64 = 2;
/// The parser `prog` of `sqb review`, and the option that makes it print a JSON document.
pub(crate) const REVIEW_PROG: &str = "sqb review";
pub(crate) const JSON_OPTION: &str = "--json";
/// Commands whose usage errors exit with their own status, where 2 means something else.
pub(crate) const USAGE_ERROR_EXITS: [(&str, i64); 1] = [(REVIEW_PROG, 3)];
pub(crate) const HELP_EXIT: i64 = 0;
pub(crate) const INDENT_INCREMENT: i64 = 2;
pub(crate) const MAX_HELP_POSITION: i64 = 24;
pub(crate) const MINIMUM_TEXT_WIDTH: i64 = 11;
/// `_long_break_matcher` folds longer runs of newlines into this many.
pub(crate) const MAXIMUM_BLANK_RUN: usize = 2;

/// `CliNamespace` attributes the post-parse rules read and write.
pub(crate) const COMMAND_DEST: &str = "command";
pub(crate) const BUILD_COMMAND: &str = "build";
pub(crate) const CONCURRENCY_DEST: &str = "concurrency";
pub(crate) const DBT_COMMAND: &str = "dbt";
pub(crate) const DBT_DEBUG_COMMAND: &str = "debug";
pub(crate) const DBT_PASSTHROUGH_COMMANDS: [&str; 4] = ["plan", "run", "build", "debug"];
pub(crate) const DBT_PASSTHROUGH_SEPARATOR: &str = "--";
pub(crate) const DBT_VERBOSE_OPTIONS: [&str; 2] = ["--verbose", "-v"];
pub(crate) const DBT_VERBOSE_OPTION: &str = "--verbose";
pub(crate) const NO_COLOR_OPTION: &str = "--no-color";
pub(crate) const DEBUG_OPTION: &str = "--debug";
pub(crate) const POSITIONAL_SELECT_COMMANDS: [&str; 8] = [
    "audit",
    "build",
    "check",
    "compile",
    "freshness",
    "plan",
    "seed",
    "test",
];
pub(crate) const CONCURRENCY_COMMANDS: [&str; 5] = ["audit", "build", "load", "seed", "test"];
pub(crate) const CURSOR_COMMANDS: [&str; 3] = ["build", "load", "plan"];
pub(crate) const RENAME_COMMAND: &str = "rename";
pub(crate) const SCOPE_COMMAND: &str = "scope";
pub(crate) const TEST_COMMAND: &str = "test";
pub(crate) const JANITOR_COMMAND: &str = "janitor";
pub(crate) const CONCURRENCY_ENV_VAR: &str = "SQLBUILD_CONCURRENCY";
pub(crate) const SCOPE_GLOBAL_SUMMARY: &str = "summary";
pub(crate) const SCOPE_DEFAULT_PAGE_SIZE: i64 = 100;
pub(crate) const SCOPE_KIND_DEST: &str = "scope_kind";
pub(crate) const INT_CONVERTER: &str = "int";
/// The `type=pathlib.Path` converter, which never rejects a string.
pub(crate) const PATH_CONVERTER: &str = "Path";

/// Every `CliNamespace` attribute with a default, in declaration order.
pub(crate) const CLI_NAMESPACE_DEFAULTS: [(&str, NamespaceDefault); 184] = [
    ("command", NamespaceDefault::None),
    ("project_dir", NamespaceDefault::None),
    ("dbt_project_dir", NamespaceDefault::None),
    ("dbt_profiles_dir", NamespaceDefault::None),
    ("dbt_target", NamespaceDefault::None),
    ("dbt_profile", NamespaceDefault::None),
    ("sqb_output_dir", NamespaceDefault::None),
    ("dry_run", NamespaceDefault::Bool(false)),
    ("overwrite", NamespaceDefault::Bool(false)),
    ("skip_dbt_debug", NamespaceDefault::Bool(false)),
    ("no_sql_validation", NamespaceDefault::Bool(false)),
    ("no_cache", NamespaceDefault::Bool(false)),
    ("defer_to", NamespaceDefault::None),
    ("defer_clone_from", NamespaceDefault::None),
    ("defer_sources_to", NamespaceDefault::None),
    ("target", NamespaceDefault::None),
    ("warehouse", NamespaceDefault::None),
    ("case", NamespaceDefault::None),
    ("inspect", NamespaceDefault::Bool(false)),
    ("since", NamespaceDefault::None),
    ("explain", NamespaceDefault::Bool(false)),
    ("review_against", NamespaceDefault::None),
    ("review_downstream", NamespaceDefault::None),
    ("review_no_clone", NamespaceDefault::Bool(false)),
    ("review_markdown", NamespaceDefault::None),
    ("target_range", NamespaceDefault::None),
    ("from_target", NamespaceDefault::None),
    ("to_target", NamespaceDefault::None),
    ("hard_copy", NamespaceDefault::Bool(false)),
    ("json", NamespaceDefault::Bool(false)),
    ("json_output", NamespaceDefault::None),
    ("manifest", NamespaceDefault::Bool(false)),
    ("dag", NamespaceDefault::None),
    ("compile_lineage_mode", NamespaceDefault::CompileLineageFast),
    (
        "profile_skip_discovery_sql_analysis",
        NamespaceDefault::Bool(false),
    ),
    (
        "profile_skip_column_inference",
        NamespaceDefault::Bool(false),
    ),
    ("profile_skip_contracts", NamespaceDefault::Bool(false)),
    ("profile_skip_write", NamespaceDefault::Bool(false)),
    ("start_cursor", NamespaceDefault::None),
    ("end_cursor", NamespaceDefault::None),
    ("start_cursor_ts", NamespaceDefault::None),
    ("max_microbatches", NamespaceDefault::None),
    ("end_cursor_ts", NamespaceDefault::None),
    ("start_cursor_int", NamespaceDefault::None),
    ("end_cursor_int", NamespaceDefault::None),
    ("no_color", NamespaceDefault::Bool(false)),
    ("fail_fast", NamespaceDefault::Bool(false)),
    ("full_refresh", NamespaceDefault::Bool(false)),
    ("as_target", NamespaceDefault::None),
    ("include_python", NamespaceDefault::Bool(true)),
    ("load_sources", NamespaceDefault::None),
    ("reload", NamespaceDefault::Bool(false)),
    ("run_tests", NamespaceDefault::Bool(true)),
    ("run_audits", NamespaceDefault::Bool(true)),
    ("allow_snapshot_full_refresh", NamespaceDefault::Bool(false)),
    ("allow_table_type_downgrade", NamespaceDefault::Bool(false)),
    ("allow_retention_decrease", NamespaceDefault::Bool(false)),
    (
        "allow_missing_migration_origin",
        NamespaceDefault::Bool(false),
    ),
    (
        "allow_snapshot_schema_change",
        NamespaceDefault::Bool(false),
    ),
    ("concurrency", NamespaceDefault::None),
    ("verbose", NamespaceDefault::Bool(false)),
    ("debug", NamespaceDefault::Bool(false)),
    ("auto_approve", NamespaceDefault::Bool(false)),
    ("retention_days", NamespaceDefault::None),
    ("direct_state_history_versions", NamespaceDefault::None),
    ("drop_old_name_views", NamespaceDefault::EmptyList),
    ("keep_in_use", NamespaceDefault::Bool(false)),
    ("bounded", NamespaceDefault::None),
    ("max_column_examples", NamespaceDefault::None),
    ("max_row_only_examples", NamespaceDefault::None),
    ("sample_rows", NamespaceDefault::None),
    ("sample_seed", NamespaceDefault::None),
    ("exhaustive", NamespaceDefault::Bool(false)),
    ("max_models", NamespaceDefault::None),
    ("max_columns", NamespaceDefault::None),
    ("query_sql", NamespaceDefault::None),
    ("query_file", NamespaceDefault::None),
    ("query_format", NamespaceDefault::None),
    ("query_limit", NamespaceDefault::Int(20)),
    ("query_no_limit", NamespaceDefault::Bool(false)),
    ("cost_selector", NamespaceDefault::Text("latest")),
    ("cost_limit", NamespaceDefault::None),
    ("cost_no_limit", NamespaceDefault::Bool(false)),
    ("cost_sort", NamespaceDefault::None),
    ("cost_order", NamespaceDefault::None),
    ("cost_since", NamespaceDefault::None),
    ("cost_until", NamespaceDefault::None),
    ("lineage_targets", NamespaceDefault::EmptyList),
    ("lineage_format", NamespaceDefault::Text("tree")),
    ("lineage_direction", NamespaceDefault::None),
    ("lineage_depth", NamespaceDefault::Text("all")),
    ("lineage_mode", NamespaceDefault::ColumnLineageRich),
    ("lineage_include_uses", NamespaceDefault::Bool(false)),
    ("no_connection", NamespaceDefault::Bool(false)),
    ("fail_on_error", NamespaceDefault::Bool(false)),
    ("fail_on_stale", NamespaceDefault::Bool(false)),
    ("state", NamespaceDefault::Bool(false)),
    ("full", NamespaceDefault::Bool(false)),
    ("schema_only", NamespaceDefault::Bool(false)),
    ("left_query", NamespaceDefault::None),
    ("left_query_file", NamespaceDefault::None),
    ("right_query", NamespaceDefault::None),
    ("right_query_file", NamespaceDefault::None),
    ("left_label", NamespaceDefault::None),
    ("right_label", NamespaceDefault::None),
    ("key", NamespaceDefault::EmptyList),
    ("unkeyed", NamespaceDefault::Bool(false)),
    ("exclude_column", NamespaceDefault::EmptyList),
    ("tolerance", NamespaceDefault::EmptyList),
    ("max_value_length", NamespaceDefault::None),
    ("no_example_values", NamespaceDefault::Bool(false)),
    ("full_example_values", NamespaceDefault::Bool(false)),
    (
        "playground_path",
        NamespaceDefault::Text("sqlbuild-playground"),
    ),
    ("playground_template", NamespaceDefault::Text("waffle_shop")),
    ("scenario_command", NamespaceDefault::None),
    ("scenario_selector", NamespaceDefault::EmptyList),
    ("scenario_retain", NamespaceDefault::Bool(false)),
    ("scenario_namespace", NamespaceDefault::None),
    ("scenario_local", NamespaceDefault::Bool(false)),
    ("scenario_strict", NamespaceDefault::Bool(false)),
    ("scenario_sync_snapshots", NamespaceDefault::Bool(false)),
    ("scenario_refresh", NamespaceDefault::Bool(false)),
    ("scenario_force", NamespaceDefault::Bool(false)),
    ("scenario_max_snapshot_rows", NamespaceDefault::None),
    ("scenario_max_snapshot_total_rows", NamespaceDefault::None),
    ("scenario_max_snapshot_bytes", NamespaceDefault::None),
    ("scenario_max_snapshot_total_bytes", NamespaceDefault::None),
    ("skills_global", NamespaceDefault::Bool(false)),
    ("skills_target", NamespaceDefault::EmptyList),
    ("skills_force", NamespaceDefault::Bool(false)),
    ("select", NamespaceDefault::EmptyList),
    ("positional_select", NamespaceDefault::EmptyList),
    ("select_file", NamespaceDefault::EmptyList),
    ("exclude", NamespaceDefault::EmptyList),
    ("dbt_command", NamespaceDefault::None),
    ("dbt_args", NamespaceDefault::EmptyList),
    ("vars", NamespaceDefault::EmptyDict),
    ("rules_command", NamespaceDefault::None),
    ("rules_rule_selector", NamespaceDefault::None),
    ("rules_skills_check", NamespaceDefault::Bool(false)),
    ("recall_command", NamespaceDefault::None),
    ("recall_sql", NamespaceDefault::None),
    ("recall_backend", NamespaceDefault::Text("logs")),
    ("recall_format", NamespaceDefault::None),
    ("recall_limit", NamespaceDefault::Int(100)),
    ("recall_no_limit", NamespaceDefault::Bool(false)),
    ("recall_max_cell_bytes", NamespaceDefault::None),
    ("recall_state", NamespaceDefault::Bool(false)),
    ("recall_relation", NamespaceDefault::None),
    ("recall_ddl", NamespaceDefault::Bool(false)),
    ("recall_invocation", NamespaceDefault::Text("last")),
    ("recall_stream", NamespaceDefault::Text("combined")),
    ("recall_tail", NamespaceDefault::None),
    ("format_check", NamespaceDefault::Bool(false)),
    ("format_diff", NamespaceDefault::Bool(false)),
    ("format_fixtures_only", NamespaceDefault::Bool(false)),
    ("format_fix", NamespaceDefault::Bool(false)),
    ("format_paths", NamespaceDefault::EmptyList),
    ("scope_target", NamespaceDefault::None),
    ("scope_at", NamespaceDefault::None),
    ("scope_as_path", NamespaceDefault::None),
    ("scope_browse", NamespaceDefault::None),
    ("scope_list", NamespaceDefault::None),
    ("contract_command", NamespaceDefault::None),
    ("contract_from", NamespaceDefault::None),
    ("contract_write", NamespaceDefault::Bool(false)),
    ("scope_defined_under", NamespaceDefault::None),
    ("scope_kind", NamespaceDefault::EmptyList),
    ("scope_match", NamespaceDefault::None),
    ("scope_used_only", NamespaceDefault::Bool(false)),
    ("scope_include_nearby", NamespaceDefault::Bool(false)),
    ("scope_nearby_depth", NamespaceDefault::Int(1)),
    ("scope_dependency_depth", NamespaceDefault::Int(0)),
    ("scope_explain", NamespaceDefault::None),
    ("scope_globals", NamespaceDefault::Text("summary")),
    ("scope_page_size", NamespaceDefault::Int(100)),
    ("scope_after", NamespaceDefault::None),
    ("scope_paths", NamespaceDefault::Text("relative")),
    ("refactor_target", NamespaceDefault::None),
    ("refactor_new_name", NamespaceDefault::None),
    ("refactor_destination", NamespaceDefault::None),
    ("refactor_more_moves", NamespaceDefault::EmptyList),
    ("refactor_from_file", NamespaceDefault::None),
    ("refactor_cascade", NamespaceDefault::Bool(false)),
];
