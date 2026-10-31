//! Every `sqb` command's handler, implementation and arguments.

use crate::dispatch::_helpers::routes::{attr, class, dbt_route, field, request, routed, same};
use crate::dispatch::models::{CallShape, ClassRef, DiffSide, Field, Route, Source};

/// Commands that create a project instead of reading one.
pub(crate) const PROJECT_CREATING_COMMANDS: [&str; 2] = ["init", "playground"];
/// Commands that never check installed skill freshness first.
pub(crate) const SKILL_EXEMPT_COMMANDS: [&str; 3] = ["init", "playground", "skills"];
pub(crate) const DBT_COMMAND_NAME: &str = "dbt";
pub(crate) const DBT_INIT_SUBCOMMAND: &str = "init";
pub(crate) const DIFF_COMMAND_NAME: &str = "diff";
/// `sqb recall` reads history and is never recorded itself.
pub(crate) const RECALL_COMMAND_NAME: &str = "recall";
/// A `--select-file` line starting with this is a comment.
pub(crate) const SELECTOR_COMMENT_PREFIX: &str = "#";

const PROJECT_DIR: Field = field("project_dir", Source::ProjectDir);
const SELECT: Field = field("select", Source::Select);
const EXCLUDE: Field = field("exclude", Source::TupleOf("exclude"));
const NO_COLOR: Field = same("no_color");
const NO_SQL_VALIDATION: Field = same("no_sql_validation");
const CLI_VARS: Field = attr("cli_vars", "vars");
const JSON_OUTPUT: Field = attr("json_output", "json");
const JSON_OUTPUT_PATH: Field = attr("json_output_path", "json_output");
const SELECTED_TARGET: Field = attr("selected_target", "target");

const CURSOR_FIELDS: [Field; 6] = [
    attr("start_ts", "start_cursor_ts"),
    attr("end_ts", "end_cursor_ts"),
    attr("start_int", "start_cursor_int"),
    attr("end_int", "end_cursor_int"),
    attr("start", "start_cursor"),
    attr("end", "end_cursor"),
];
const CURSOR_OVERRIDES: Field = field(
    "cursor_overrides",
    Source::Nested(class("CursorOverrides"), &CURSOR_FIELDS),
);

const SNAPSHOT_LIMIT_FIELDS: [Field; 5] = [
    attr("max_snapshot_rows", "scenario_max_snapshot_rows"),
    attr(
        "max_snapshot_total_rows",
        "scenario_max_snapshot_total_rows",
    ),
    attr("max_snapshot_bytes", "scenario_max_snapshot_bytes"),
    attr(
        "max_snapshot_total_bytes",
        "scenario_max_snapshot_total_bytes",
    ),
    attr("force", "scenario_force"),
];
const SNAPSHOT_LIMITS: Field = field(
    "limit_inputs",
    Source::Nested(class("ScenarioSnapshotLimitInputs"), &SNAPSHOT_LIMIT_FIELDS),
);

/// A handler receiving one request positionally.
const REQUEST_SHAPE: CallShape = CallShape {
    slot_positional: 1,
    implementation_keywords: false,
};
/// A handler receiving `request=` by keyword.
const REQUEST_KEYWORD_SHAPE: CallShape = CallShape {
    slot_positional: 0,
    implementation_keywords: true,
};

const SCOPE_FIELDS: [Field; 22] = [
    PROJECT_DIR,
    attr("target", "scope_target"),
    attr("at", "scope_at"),
    attr("as_path", "scope_as_path"),
    attr("browse", "scope_browse"),
    attr("list_path", "scope_list"),
    attr("defined_under", "scope_defined_under"),
    field("kinds", Source::TupleOf("scope_kind")),
    attr("match", "scope_match"),
    attr("used_only", "scope_used_only"),
    attr("include_nearby", "scope_include_nearby"),
    attr("nearby_depth", "scope_nearby_depth"),
    attr("dependency_depth", "scope_dependency_depth"),
    attr("explain", "scope_explain"),
    attr("globals", "scope_globals"),
    attr("page_size", "scope_page_size"),
    attr("after", "scope_after"),
    attr("paths", "scope_paths"),
    JSON_OUTPUT,
    same("no_cache"),
    same("verbose"),
    NO_COLOR,
];
const SCOPE_ARGUMENTS: [Field; 1] = [request(class("ScopeCommandRequest"), &SCOPE_FIELDS)];
pub(crate) const SCOPE_ROUTE: Route = routed(
    "run_scope",
    "run_scope_command",
    &SCOPE_ARGUMENTS,
    REQUEST_KEYWORD_SHAPE,
);

const DAG_ARGUMENTS: [Field; 4] = [PROJECT_DIR, NO_SQL_VALIDATION, JSON_OUTPUT, CLI_VARS];
pub(crate) const DAG_ROUTE: Route = routed(
    "run_dag",
    "run_dag_command",
    &DAG_ARGUMENTS,
    CallShape {
        slot_positional: 0,
        implementation_keywords: true,
    },
);

const PLAN_FIELDS: [Field; 19] = [
    PROJECT_DIR,
    NO_SQL_VALIDATION,
    same("no_cache"),
    same("defer_to"),
    same("defer_sources_to"),
    SELECTED_TARGET,
    same("as_target"),
    CURSOR_OVERRIDES,
    JSON_OUTPUT,
    same("full_refresh"),
    same("load_sources"),
    same("include_python"),
    NO_COLOR,
    SELECT,
    EXCLUDE,
    same("verbose"),
    CLI_VARS,
    same("max_microbatches"),
    same("selection_diagnostics"),
];
const PLAN_ARGUMENTS: [Field; 1] = [request(class("PlanCommandRequest"), &PLAN_FIELDS)];
pub(crate) const PLAN_ROUTE: Route = routed(
    "run_plan",
    "run_plan_command",
    &PLAN_ARGUMENTS,
    REQUEST_KEYWORD_SHAPE,
);

const BUILD_FIELDS: [Field; 35] = [
    PROJECT_DIR,
    NO_SQL_VALIDATION,
    same("no_cache"),
    same("defer_to"),
    same("defer_clone_from"),
    same("defer_sources_to"),
    SELECTED_TARGET,
    CURSOR_OVERRIDES,
    NO_COLOR,
    same("fail_fast"),
    same("full_refresh"),
    same("load_sources"),
    attr("reload_sources", "reload"),
    same("include_python"),
    same("allow_snapshot_full_refresh"),
    same("allow_table_type_downgrade"),
    same("allow_retention_decrease"),
    same("allow_missing_migration_origin"),
    same("allow_snapshot_schema_change"),
    same("concurrency"),
    SELECT,
    field("selector_files", Source::SelectorFiles),
    EXCLUDE,
    same("verbose"),
    same("debug"),
    CLI_VARS,
    same("run_tests"),
    same("run_audits"),
    same("manifest"),
    JSON_OUTPUT,
    JSON_OUTPUT_PATH,
    attr("event_output_path", "event_output"),
    same("max_microbatches"),
    same("selection_diagnostics"),
    same("test_timeout"),
];
const BUILD_ARGUMENTS: [Field; 1] = [request(class("BuildCommandRequest"), &BUILD_FIELDS)];
pub(crate) const BUILD_ROUTE: Route = routed(
    "run_build",
    "run_build_command",
    &BUILD_ARGUMENTS,
    REQUEST_KEYWORD_SHAPE,
);

const FRESHNESS_FIELDS: [Field; 12] = [
    PROJECT_DIR,
    NO_SQL_VALIDATION,
    NO_COLOR,
    SELECTED_TARGET,
    SELECT,
    EXCLUDE,
    CLI_VARS,
    JSON_OUTPUT,
    JSON_OUTPUT_PATH,
    same("fail_on_error"),
    attr("compare_state", "state"),
    same("fail_on_stale"),
];
const FRESHNESS_ARGUMENTS: [Field; 1] =
    [request(class("FreshnessCommandRequest"), &FRESHNESS_FIELDS)];
pub(crate) const FRESHNESS_ROUTE: Route = routed(
    "run_freshness",
    "run_freshness_command",
    &FRESHNESS_ARGUMENTS,
    REQUEST_KEYWORD_SHAPE,
);

const TEST_FIELDS: [Field; 15] = [
    PROJECT_DIR,
    NO_SQL_VALIDATION,
    NO_COLOR,
    SELECTED_TARGET,
    SELECT,
    EXCLUDE,
    CLI_VARS,
    JSON_OUTPUT,
    JSON_OUTPUT_PATH,
    same("concurrency"),
    attr("case_name", "case"),
    same("inspect"),
    same("test_timeout"),
    same("since"),
    same("explain"),
];
const TEST_ARGUMENTS: [Field; 1] = [request(class("TestCommandRequest"), &TEST_FIELDS)];
pub(crate) const TEST_ROUTE: Route = routed(
    "run_test",
    "run_test_command",
    &TEST_ARGUMENTS,
    REQUEST_KEYWORD_SHAPE,
);

const CHECK_FIELDS: [Field; 9] = [
    PROJECT_DIR,
    NO_SQL_VALIDATION,
    NO_COLOR,
    SELECTED_TARGET,
    SELECT,
    EXCLUDE,
    CLI_VARS,
    JSON_OUTPUT,
    JSON_OUTPUT_PATH,
];
const CHECK_ARGUMENTS: [Field; 1] = [request(class("CheckCommandRequest"), &CHECK_FIELDS)];
pub(crate) const CHECK_ROUTE: Route = routed(
    "run_check",
    "run_check_command",
    &CHECK_ARGUMENTS,
    REQUEST_KEYWORD_SHAPE,
);

const AUDIT_FIELDS: [Field; 11] = [
    PROJECT_DIR,
    NO_SQL_VALIDATION,
    same("defer_to"),
    NO_COLOR,
    SELECTED_TARGET,
    same("concurrency"),
    SELECT,
    EXCLUDE,
    CLI_VARS,
    JSON_OUTPUT,
    JSON_OUTPUT_PATH,
];
const AUDIT_ARGUMENTS: [Field; 1] = [request(class("AuditCommandRequest"), &AUDIT_FIELDS)];
pub(crate) const AUDIT_ROUTE: Route = routed(
    "run_audit",
    "run_audit_command",
    &AUDIT_ARGUMENTS,
    REQUEST_KEYWORD_SHAPE,
);

const LOAD_FIELDS: [Field; 11] = [
    PROJECT_DIR,
    NO_COLOR,
    SELECTED_TARGET,
    SELECT,
    EXCLUDE,
    same("reload"),
    same("concurrency"),
    CURSOR_OVERRIDES,
    CLI_VARS,
    JSON_OUTPUT,
    JSON_OUTPUT_PATH,
];
const LOAD_ARGUMENTS: [Field; 1] = [request(class("LoadCommandRequest"), &LOAD_FIELDS)];
pub(crate) const LOAD_ROUTE: Route = routed(
    "run_load",
    "run_load_command",
    &LOAD_ARGUMENTS,
    REQUEST_KEYWORD_SHAPE,
);

const SEED_FIELDS: [Field; 9] = [
    PROJECT_DIR,
    NO_COLOR,
    SELECTED_TARGET,
    SELECT,
    EXCLUDE,
    same("concurrency"),
    CLI_VARS,
    JSON_OUTPUT,
    JSON_OUTPUT_PATH,
];
const SEED_ARGUMENTS: [Field; 1] = [request(class("SeedCommandRequest"), &SEED_FIELDS)];
pub(crate) const SEED_ROUTE: Route = routed(
    "run_seed",
    "run_seed_command",
    &SEED_ARGUMENTS,
    REQUEST_KEYWORD_SHAPE,
);

const LINEAGE_FIELDS: [Field; 11] = [
    PROJECT_DIR,
    NO_SQL_VALIDATION,
    field("targets", Source::TupleOf("lineage_targets")),
    attr("output_format", "lineage_format"),
    attr("direction", "lineage_direction"),
    attr("depth", "lineage_depth"),
    SELECT,
    EXCLUDE,
    field(
        "lineage_mode",
        Source::Member(class("ColumnLineageMode"), "lineage_mode"),
    ),
    attr("include_uses", "lineage_include_uses"),
    CLI_VARS,
];
const LINEAGE_ARGUMENTS: [Field; 1] = [request(class("LineageCommandRequest"), &LINEAGE_FIELDS)];
pub(crate) const LINEAGE_ROUTE: Route = routed(
    "run_lineage",
    "run_lineage_command",
    &LINEAGE_ARGUMENTS,
    REQUEST_KEYWORD_SHAPE,
);

const CLONE_FIELDS: [Field; 12] = [
    PROJECT_DIR,
    NO_COLOR,
    NO_SQL_VALIDATION,
    attr("origin_target_name", "from_target"),
    attr("destination_target_name", "to_target"),
    same("hard_copy"),
    SELECT,
    EXCLUDE,
    same("verbose"),
    CLI_VARS,
    JSON_OUTPUT_PATH,
    attr("event_output_path", "event_output"),
];
const CLONE_ARGUMENTS: [Field; 1] = [request(class("CloneCommandRequest"), &CLONE_FIELDS)];
pub(crate) const CLONE_ROUTE: Route = routed(
    "run_clone",
    "run_clone_command",
    &CLONE_ARGUMENTS,
    REQUEST_KEYWORD_SHAPE,
);

const DIFF_FIELDS: [Field; 35] = [
    PROJECT_DIR,
    NO_COLOR,
    NO_SQL_VALIDATION,
    field("from_name", Source::DiffName(DiffSide::From)),
    field("to_name", Source::DiffName(DiffSide::To)),
    same("full"),
    same("schema_only"),
    same("bounded"),
    same("max_column_examples"),
    same("max_row_only_examples"),
    same("sample_rows"),
    same("sample_seed"),
    same("exhaustive"),
    JSON_OUTPUT,
    JSON_OUTPUT_PATH,
    same("max_models"),
    same("max_columns"),
    SELECT,
    EXCLUDE,
    same("verbose"),
    CLI_VARS,
    SELECTED_TARGET,
    same("left_query"),
    same("left_query_file"),
    same("left_label"),
    same("right_query"),
    same("right_query_file"),
    same("right_label"),
    field("unique_key_override", Source::TupleOf("key")),
    same("unkeyed"),
    field(
        "excluded_columns_override",
        Source::TupleOf("exclude_column"),
    ),
    field("tolerance_overrides", Source::TupleOf("tolerance")),
    same("max_value_length"),
    attr("suppress_example_values", "no_example_values"),
    same("full_example_values"),
];
const DIFF_ARGUMENTS: [Field; 1] = [request(class("DiffCommandRequest"), &DIFF_FIELDS)];
pub(crate) const DIFF_ROUTE: Route = routed(
    "run_diff",
    "run_diff_command",
    &DIFF_ARGUMENTS,
    REQUEST_KEYWORD_SHAPE,
);

const REVIEW_FIELDS: [Field; 17] = [
    PROJECT_DIR,
    NO_COLOR,
    NO_SQL_VALIDATION,
    SELECTED_TARGET,
    SELECT,
    CLI_VARS,
    JSON_OUTPUT,
    same("since"),
    attr("against", "review_against"),
    attr("downstream", "review_downstream"),
    same("dry_run"),
    attr("no_clone", "review_no_clone"),
    same("bounded"),
    attr("markdown_path", "review_markdown"),
    same("concurrency"),
    same("verbose"),
    same("debug"),
];
const REVIEW_ARGUMENTS: [Field; 1] = [request(class("ReviewCommandRequest"), &REVIEW_FIELDS)];
pub(crate) const REVIEW_ROUTE: Route = routed(
    "run_review",
    "run_review_command",
    &REVIEW_ARGUMENTS,
    REQUEST_KEYWORD_SHAPE,
);

const QUERY_ARGUMENTS: [Field; 6] = [
    PROJECT_DIR,
    attr("sql", "query_sql"),
    field("query_file", Source::OptionalPath("query_file")),
    SELECTED_TARGET,
    attr("output_format", "query_format"),
    field("limit", Source::QueryLimit),
];
pub(crate) const QUERY_ROUTE: Route = routed(
    "run_query",
    "run_query_command",
    &QUERY_ARGUMENTS,
    CallShape {
        slot_positional: 0,
        implementation_keywords: true,
    },
);

const RECALL_ARGUMENTS: [Field; 14] = [
    PROJECT_DIR,
    SELECTED_TARGET,
    same("recall_command"),
    attr("sql", "recall_sql"),
    attr("backend", "recall_backend"),
    attr("output_format", "recall_format"),
    attr("limit", "recall_limit"),
    attr("no_limit", "recall_no_limit"),
    attr("max_cell_bytes", "recall_max_cell_bytes"),
    attr("relation", "recall_relation"),
    attr("invocation", "recall_invocation"),
    attr("stream", "recall_stream"),
    attr("tail", "recall_tail"),
    attr("ddl", "recall_ddl"),
];
pub(crate) const RECALL_ROUTE: Route = routed(
    "run_recall",
    "run_recall_command",
    &RECALL_ARGUMENTS,
    CallShape {
        slot_positional: 0,
        implementation_keywords: true,
    },
);

const COST_FIELDS: [Field; 11] = [
    PROJECT_DIR,
    attr("selector", "cost_selector"),
    NO_COLOR,
    attr("limit", "cost_limit"),
    attr("no_limit", "cost_no_limit"),
    attr("sort", "cost_sort"),
    attr("order", "cost_order"),
    attr("since", "cost_since"),
    attr("until", "cost_until"),
    JSON_OUTPUT,
    JSON_OUTPUT_PATH,
];
const COST_ARGUMENTS: [Field; 1] = [request(class("CostCommandRequest"), &COST_FIELDS)];
pub(crate) const COST_ROUTE: Route = routed(
    "run_cost",
    "run_cost_command",
    &COST_ARGUMENTS,
    REQUEST_SHAPE,
);

const DEBUG_ARGUMENTS: [Field; 5] = [
    PROJECT_DIR,
    NO_COLOR,
    same("no_connection"),
    SELECTED_TARGET,
    JSON_OUTPUT,
];
pub(crate) const DEBUG_ROUTE: Route = routed(
    "run_debug",
    "run_debug_command",
    &DEBUG_ARGUMENTS,
    CallShape {
        slot_positional: 0,
        implementation_keywords: true,
    },
);

const LOCATION_ARGUMENTS: [Field; 9] = [
    field("project_dir", Source::EffectiveProjectDir),
    SELECTED_TARGET,
    same("location_id"),
    attr("server_url", "location_server_url"),
    attr("generation", "location_generation"),
    attr("image_id", "location_image_id"),
    attr("action", "location_command"),
    attr("run_id", "location_run_id"),
    attr("claim_id", "location_claim_id"),
];
pub(crate) const LOCATION_ROUTE: Route = routed(
    "run_location",
    "run_location_command",
    &LOCATION_ARGUMENTS,
    CallShape {
        slot_positional: 0,
        implementation_keywords: true,
    },
);

const JANITOR_FIELDS: [Field; 11] = [
    PROJECT_DIR,
    NO_COLOR,
    same("auto_approve"),
    same("retention_days"),
    same("direct_state_history_versions"),
    field(
        "drop_old_name_views",
        Source::TupleOf("drop_old_name_views"),
    ),
    same("keep_in_use"),
    SELECTED_TARGET,
    same("as_target"),
    same("verbose"),
    JSON_OUTPUT,
];
const JANITOR_ARGUMENTS: [Field; 1] = [request(class("JanitorCommandRequest"), &JANITOR_FIELDS)];
pub(crate) const JANITOR_ROUTE: Route = routed(
    "run_janitor",
    "run_janitor_command",
    &JANITOR_ARGUMENTS,
    REQUEST_KEYWORD_SHAPE,
);

const REFACTOR_FIELDS: [Field; 11] = [
    field("command", Source::Text("command")),
    attr("target", "refactor_target"),
    attr("new_name", "refactor_new_name"),
    attr("destination", "refactor_destination"),
    field("more_moves", Source::TupleOf("refactor_more_moves")),
    attr("from_file", "refactor_from_file"),
    PROJECT_DIR,
    attr("cascade", "refactor_cascade"),
    same("dry_run"),
    JSON_OUTPUT,
    NO_COLOR,
];
const REFACTOR_ARGUMENTS: [Field; 1] = [request(class("RefactorCommandRequest"), &REFACTOR_FIELDS)];
pub(crate) const REFACTOR_ROUTE: Route = Route {
    unavailable_code: Some("C950"),
    ..routed(
        "run_refactor",
        "run_refactor_command",
        &REFACTOR_ARGUMENTS,
        REQUEST_KEYWORD_SHAPE,
    )
};

const CONTRACT_FIELDS: [Field; 10] = [
    attr("action", "contract_command"),
    attr("from_target", "contract_from"),
    PROJECT_DIR,
    SELECT,
    EXCLUDE,
    attr("write", "contract_write"),
    same("overwrite"),
    JSON_OUTPUT,
    NO_COLOR,
    CLI_VARS,
];
const CONTRACT_ARGUMENTS: [Field; 1] = [request(class("ContractCommandRequest"), &CONTRACT_FIELDS)];
pub(crate) const CONTRACT_ROUTE: Route = Route {
    unavailable_code: Some("C470"),
    ..routed(
        "run_contract",
        "run_contract_command",
        &CONTRACT_ARGUMENTS,
        REQUEST_KEYWORD_SHAPE,
    )
};

const PROFILE_FIELDS: [Field; 4] = [
    attr(
        "skip_discovery_sql_analysis",
        "profile_skip_discovery_sql_analysis",
    ),
    attr("skip_column_inference", "profile_skip_column_inference"),
    attr("skip_contracts", "profile_skip_contracts"),
    attr("skip_write", "profile_skip_write"),
];
const COMPILE_FIELDS: [Field; 15] = [
    PROJECT_DIR,
    NO_SQL_VALIDATION,
    same("no_cache"),
    same("defer_to"),
    SELECTED_TARGET,
    JSON_OUTPUT,
    same("manifest"),
    attr("dag_path", "dag"),
    NO_COLOR,
    field(
        "lineage_mode",
        Source::Member(class("CompileLineageMode"), "compile_lineage_mode"),
    ),
    SELECT,
    EXCLUDE,
    CLI_VARS,
    field(
        "profile_flags",
        Source::Nested(class("CompileProfileFlags"), &PROFILE_FIELDS),
    ),
    same("debug"),
];
const COMPILE_ARGUMENTS: [Field; 1] = [request(class("CompileCommandRequest"), &COMPILE_FIELDS)];
pub(crate) const COMPILE_ROUTE: Route = routed(
    "run_compile",
    "run_compile",
    &COMPILE_ARGUMENTS,
    REQUEST_SHAPE,
);

const INIT_ARGUMENTS: [Field; 1] = [PROJECT_DIR];
pub(crate) const INIT_ROUTE: Route = routed(
    "run_init",
    "run_init_command",
    &INIT_ARGUMENTS,
    REQUEST_SHAPE,
);

const PLAYGROUND_FIELDS: [Field; 3] = [
    PROJECT_DIR,
    attr("target_path", "playground_path"),
    attr("template", "playground_template"),
];
const PLAYGROUND_ARGUMENTS: [Field; 1] = [request(
    class("PlaygroundCommandRequest"),
    &PLAYGROUND_FIELDS,
)];
pub(crate) const PLAYGROUND_ROUTE: Route = routed(
    "run_playground",
    "run_playground_command",
    &PLAYGROUND_ARGUMENTS,
    REQUEST_SHAPE,
);

const SKILLS_ARGUMENTS: [Field; 4] = [
    PROJECT_DIR,
    attr("global_install", "skills_global"),
    field("targets", Source::TupleOf("skills_target")),
    attr("force", "skills_force"),
];
pub(crate) const SKILLS_ROUTE: Route = routed(
    "run_skills_update",
    "run_skills_command",
    &SKILLS_ARGUMENTS,
    CallShape {
        slot_positional: 0,
        implementation_keywords: true,
    },
);

const FORMAT_ARGUMENTS: [Field; 10] = [
    PROJECT_DIR,
    field("select", Source::FormatSelect),
    EXCLUDE,
    attr("check", "format_check"),
    attr("diff", "format_diff"),
    attr("fixtures_only", "format_fixtures_only"),
    attr("fix", "format_fix"),
    same("no_cache"),
    JSON_OUTPUT,
    NO_COLOR,
];
pub(crate) const FORMAT_ROUTE: Route = routed(
    "run_format",
    "run_format_command",
    &FORMAT_ARGUMENTS,
    CallShape {
        slot_positional: 1,
        implementation_keywords: true,
    },
);

const RULES_FIELDS: [Field; 7] = [
    PROJECT_DIR,
    JSON_OUTPUT,
    field("action", Source::OrDefault("rules_command", "list")),
    attr("rule_selector", "rules_rule_selector"),
    attr("skills_check", "rules_skills_check"),
    field("select", Source::TupleOf("select")),
    EXCLUDE,
];
const RULES_ARGUMENTS: [Field; 1] = [request(class("RulesCommandRequest"), &RULES_FIELDS)];
pub(crate) const RULES_ROUTE: Route = routed(
    "run_rules",
    "run_rules_command",
    &RULES_ARGUMENTS,
    REQUEST_KEYWORD_SHAPE,
);

const DBT_INIT_FIELDS: [Field; 9] = [
    field("cwd", Source::EffectiveProjectDir),
    same("dbt_project_dir"),
    attr("profiles_dir", "dbt_profiles_dir"),
    attr("profile_name", "dbt_profile"),
    attr("target_name", "dbt_target"),
    same("sqb_output_dir"),
    same("dry_run"),
    same("overwrite"),
    same("skip_dbt_debug"),
];
const DBT_INIT_ARGUMENTS: [Field; 1] = [request(class("DbtInitCommandRequest"), &DBT_INIT_FIELDS)];
pub(crate) const DBT_INIT_ROUTE: Route = routed(
    "run_dbt_init",
    "run_dbt_init_command",
    &DBT_INIT_ARGUMENTS,
    REQUEST_SHAPE,
);

const DBT_ARGUMENTS: [Field; 3] = [
    PROJECT_DIR,
    field("args", Source::TupleOf("dbt_args")),
    NO_COLOR,
];
const DBT_COMMAND: ClassRef = class("DbtInteropCommand");

pub(crate) const DBT_PLAN_ROUTE: Route =
    dbt_route("run_dbt_plan", "plan", &DBT_ARGUMENTS, DBT_COMMAND);
pub(crate) const DBT_RUN_ROUTE: Route =
    dbt_route("run_dbt_run", "run", &DBT_ARGUMENTS, DBT_COMMAND);
pub(crate) const DBT_BUILD_ROUTE: Route =
    dbt_route("run_dbt_build", "build", &DBT_ARGUMENTS, DBT_COMMAND);
pub(crate) const DBT_DEBUG_ROUTE: Route =
    dbt_route("run_dbt_debug", "debug", &DBT_ARGUMENTS, DBT_COMMAND);

const SCENARIO_COMMON: [Field; 7] = [
    attr("scenario_namespace", "scenario_namespace"),
    PROJECT_DIR,
    field("no_sql_validation", Source::Flag(false)),
    NO_COLOR,
    field("selectors", Source::ScenarioSelect),
    EXCLUDE,
    attr("retain", "scenario_retain"),
];
const SCENARIO_TEST_FIELDS: [Field; 18] = [
    SCENARIO_COMMON[0],
    SCENARIO_COMMON[1],
    SCENARIO_COMMON[2],
    SCENARIO_COMMON[3],
    SCENARIO_COMMON[4],
    SCENARIO_COMMON[5],
    SCENARIO_COMMON[6],
    attr("local", "scenario_local"),
    attr("strict", "scenario_strict"),
    same("concurrency"),
    attr("sync_snapshots", "scenario_sync_snapshots"),
    attr("refresh", "scenario_refresh"),
    SNAPSHOT_LIMITS,
    JSON_OUTPUT,
    JSON_OUTPUT_PATH,
    same("scenario_timeout"),
    same("since"),
    same("explain"),
];
const SCENARIO_TEST_ARGUMENTS: [Field; 1] = [request(
    class("ScenarioTestCommandRequest"),
    &SCENARIO_TEST_FIELDS,
)];
pub(crate) const SCENARIO_TEST_ROUTE: Route = routed(
    "run_scenario",
    "run_scenario_test_command",
    &SCENARIO_TEST_ARGUMENTS,
    REQUEST_KEYWORD_SHAPE,
);

const SCENARIO_CAPTURE_FIELDS: [Field; 8] = [
    SCENARIO_COMMON[0],
    SCENARIO_COMMON[1],
    SCENARIO_COMMON[2],
    SCENARIO_COMMON[3],
    SCENARIO_COMMON[4],
    SCENARIO_COMMON[5],
    SCENARIO_COMMON[6],
    SNAPSHOT_LIMITS,
];
const SCENARIO_CAPTURE_ARGUMENTS: [Field; 1] = [request(
    class("ScenarioCaptureCommandRequest"),
    &SCENARIO_CAPTURE_FIELDS,
)];
pub(crate) const SCENARIO_CAPTURE_ROUTE: Route = routed(
    "run_scenario_capture",
    "run_scenario_capture_command",
    &SCENARIO_CAPTURE_ARGUMENTS,
    REQUEST_KEYWORD_SHAPE,
);

/// The setting a user raises when lifecycle event export runs out of time at exit.
pub(crate) const SHUTDOWN_TIMEOUT_SETTING: &str = "sinks.lifecycle.shutdown_timeout";
