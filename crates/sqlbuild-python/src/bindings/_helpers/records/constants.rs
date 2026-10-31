//! The layouts of every native record class.

use sqlbuild_host::entries::models::ExecutorHost;

use crate::bindings::_helpers::compile_records::constants::COMPILE_VALUE_RECORDS;
use crate::bindings::_helpers::compiler_enums::constants::COMPILER_ENUMS;
use crate::bindings::_helpers::compiler_errors::constants::COMPILER_ERRORS;
use crate::bindings::_helpers::compiler_records::constants::COMPILER_RECORDS;
use crate::bindings::_helpers::contract_records::constants::CONTRACT_RECORDS;
use crate::bindings::_helpers::planner_records::constants::{
    PLANNER_ENUMS, PLANNER_RECORDS, PLANNER_VALUE_RECORDS,
};
use crate::bindings::_helpers::records::ids::RecordId;
use crate::bindings::_helpers::records::models::{
    ClassValue, EnumSpec, ErrorSpec, FieldDefault, FieldSpec, RecordBehaviours, RecordSpec,
};
use crate::bindings::_helpers::rule_records::constants::RULE_RECORDS;
use crate::bindings::_helpers::scope_records::constants::SCOPE_RECORDS;

pub(crate) const NATIVE_MODULE: &str = "sqlbuild._native";
/// The class attribute holding a record class's index into `RECORDS`.
pub(crate) const RECORD_SPEC_ATTRIBUTE: &str = "__native_record__";
pub(crate) const FROZEN_INSTANCE_ERROR: (&str, &str) = ("dataclasses", "FrozenInstanceError");
pub(crate) const DATACLASSES_MODULE: &str = "dataclasses";
pub(crate) const DATACLASS_FIELDS_ATTRIBUTE: &str = "__dataclass_fields__";
/// The marker `dataclasses` gives ordinary (not class or init-only) fields.
pub(crate) const DATACLASS_FIELD_KIND: &str = "_FIELD";

pub(crate) const ENUM_MODULE: &str = "enum";
pub(crate) const STR_ENUM: &str = "StrEnum";

/// The class attribute holding an error class's index into `ERRORS`.
pub(crate) const ERROR_SPEC_ATTRIBUTE: &str = "__native_error__";
/// The `parent` of an error class deriving directly from `Exception`.
pub(crate) const EXCEPTION_PARENT: &str = "Exception";
/// The `parent` of an error class deriving directly from `ValueError`.
pub(crate) const VALUE_ERROR_PARENT: &str = "ValueError";
/// Prefix of a `parent` naming a builtin exception kept with its own constructor.
pub(crate) const BUILTIN_PARENT_PREFIX: &str = "builtins.";
pub(crate) const BUILTINS_MODULE: &str = "builtins";

/// Every native error group, in registration order.
pub(crate) const ERRORS: &[&[ErrorSpec]] = &[CLI_ERRORS, COMPILER_ERRORS];

/// The expected CLI and migration-state error classes, each after its parent.
const CLI_ERRORS: &[ErrorSpec] = &[
    ErrorSpec {
        name: "CliUserError",
        parent: EXCEPTION_PARENT,
        doc: "Expected CLI-facing error that should be rendered without a traceback.",
        class_attributes: &[("code", ClassValue::Text("C000"))],
        required: &[],
        optional: &[],
        fixed: &[],
    },
    ErrorSpec {
        name: "StagedArtifactsChangedError",
        parent: "CliUserError",
        doc: "Staged compile artifacts changed before publication, so nothing stale is deleted.",
        class_attributes: &[],
        required: &[],
        optional: &[],
        fixed: &[],
    },
    ErrorSpec {
        name: "QueryExecutionError",
        parent: "CliUserError",
        doc: "The warehouse rejected or failed an ad hoc query.",
        class_attributes: &[
            ("code", ClassValue::Text("C108")),
            ("exit_code", ClassValue::Int(2)),
        ],
        required: &[],
        optional: &[],
        fixed: &[],
    },
    ErrorSpec {
        name: "QueryDiffOutcomeError",
        parent: "CliUserError",
        doc: "Expected query-diff failure with an explicit outcome and exit code.",
        class_attributes: &[],
        required: &["status", "exit_code"],
        optional: &[],
        fixed: &[],
    },
    ErrorSpec {
        name: "QueryDiffIncompleteError",
        parent: "QueryDiffOutcomeError",
        doc: "Safety or preflight prevented complete value evidence.",
        class_attributes: &[],
        required: &[],
        optional: &[],
        fixed: &[
            ("status", ClassValue::Text("incomplete")),
            ("exit_code", ClassValue::Int(2)),
        ],
    },
    ErrorSpec {
        name: "QueryDiffExecutionError",
        parent: "QueryDiffOutcomeError",
        doc: "Setup, execution, ownership publication, or cleanup failed.",
        class_attributes: &[],
        required: &[],
        optional: &[],
        fixed: &[
            ("status", ClassValue::Text("execution_failed")),
            ("exit_code", ClassValue::Int(3)),
        ],
    },
    ErrorSpec {
        name: "ReviewFailedError",
        parent: "CliUserError",
        doc: "A review that could not set up, run or clean up; its report is already written.",
        class_attributes: &[
            ("code", ClassValue::Text("C760")),
            ("exit_code", ClassValue::Int(3)),
        ],
        required: &[],
        optional: &[],
        fixed: &[],
    },
    ErrorSpec {
        name: "DiffSizeGuardError",
        parent: "CliUserError",
        doc: "A default full model diff stopped before reading data because of table sizes.",
        class_attributes: &[
            ("status", ClassValue::Text("incomplete")),
            ("exit_code", ClassValue::Int(2)),
        ],
        required: &["details"],
        optional: &[],
        fixed: &[],
    },
    ErrorSpec {
        name: "MigrationStateError",
        parent: VALUE_ERROR_PARENT,
        doc: "Raised when model migration state inputs or rows are invalid.",
        class_attributes: &[("code", ClassValue::Text("M000"))],
        required: &[],
        optional: &[],
        fixed: &[],
    },
];

/// Every native string enum group, registered before the records.
pub(crate) const ENUMS: &[&[EnumSpec]] = &[
    FRESHNESS_ENUMS,
    CLI_ENUMS,
    DBT_ENUMS,
    PRESENTATION_ENUMS,
    COMPILER_ENUMS,
    PLANNER_ENUMS,
];

/// Every native record group, in registration order.
pub(crate) const RECORDS: &[&[RecordSpec]] = &[
    COMMAND_RECORDS,
    ENTRY_RECORDS,
    OUTPUT_RECORDS,
    COMPILE_RECORDS,
    DBT_RECORDS,
    FRESHNESS_RECORDS,
    COST_RECORDS,
    SQL_VALUE_RECORDS,
    DIAGNOSTIC_RECORDS,
    COMPILER_RECORDS,
    COMPILE_VALUE_RECORDS,
    SCOPE_RECORDS,
    PLANNER_RECORDS,
    PLANNER_VALUE_RECORDS,
    RULE_RECORDS,
    CONTRACT_RECORDS,
];

/// `str(PurePath("."))`, the path whose name and stem are empty.
pub(crate) const CURRENT_DIRECTORY_PATH: &str = ".";
/// The behaviours before registration installs them: none.
pub(crate) const NO_RECORD_BEHAVIOURS: RecordBehaviours = RecordBehaviours {
    finishers: &[],
    properties: &[],
    cached_properties: &[],
    methods: &[],
    ordered: &[],
    sequences: &[],
};
/// `SourceFreshnessObservation` field positions.
pub(crate) const DATA_VERSION_FIELD: usize = 2;
pub(crate) const OBSERVED_AT_FIELD: usize = 4;

const NESTED_PROGRESS_CHILD_ROW: RecordSpec = RecordSpec {
    name: "NestedProgressChildRow",
    doc: "One child row rendered below a completed nested progress item.",
    fields: &[
        FieldSpec::required("label"),
        FieldSpec::required("name"),
        FieldSpec::required("status_text"),
        FieldSpec::new("detail", FieldDefault::Text("")),
    ],
};
const AUDIT_COMMAND_REQUEST: RecordSpec = RecordSpec {
    name: "AuditCommandRequest",
    doc: "CLI inputs for one audit command invocation.",
    fields: &[
        FieldSpec::new("project_dir", FieldDefault::None),
        FieldSpec::new("no_sql_validation", FieldDefault::False),
        FieldSpec::new("defer_to", FieldDefault::None),
        FieldSpec::new("no_color", FieldDefault::False),
        FieldSpec::new("selected_target", FieldDefault::None),
        FieldSpec::new("concurrency", FieldDefault::None),
        FieldSpec::new("select", FieldDefault::EmptyTuple),
        FieldSpec::new("exclude", FieldDefault::EmptyTuple),
        FieldSpec::new("cli_vars", FieldDefault::None),
        FieldSpec::new("json_output", FieldDefault::False),
        FieldSpec::new("json_output_path", FieldDefault::None),
    ],
};
const AUDIT_INVOCATION: RecordSpec = RecordSpec {
    name: "AuditInvocation",
    doc: "Resolved project, adapter, and reporter context for the audit command.",
    fields: &[
        FieldSpec::required("effective_project_dir"),
        FieldSpec::required("discovered_inputs"),
        FieldSpec::required("adapter_name"),
        FieldSpec::required("adapter"),
        FieldSpec::required("connection_config"),
        FieldSpec::required("use_color"),
        FieldSpec::required("progress_stream"),
        FieldSpec::required("connection_progress"),
        FieldSpec::required("planning_progress"),
    ],
};
const BUILD_COMMAND_REQUEST: RecordSpec = RecordSpec {
    name: "BuildCommandRequest",
    doc: "CLI inputs for one build command invocation.",
    fields: &[
        FieldSpec::new("project_dir", FieldDefault::None),
        FieldSpec::new("no_sql_validation", FieldDefault::False),
        FieldSpec::new("defer_to", FieldDefault::None),
        FieldSpec::new("defer_clone_from", FieldDefault::None),
        FieldSpec::new("defer_sources_to", FieldDefault::None),
        FieldSpec::new("selected_target", FieldDefault::None),
        FieldSpec::new("cursor_overrides", FieldDefault::None),
        FieldSpec::new("no_color", FieldDefault::False),
        FieldSpec::new("fail_fast", FieldDefault::False),
        FieldSpec::new("full_refresh", FieldDefault::False),
        FieldSpec::new("load_sources", FieldDefault::None),
        FieldSpec::new("reload_sources", FieldDefault::False),
        FieldSpec::new("include_python", FieldDefault::True),
        FieldSpec::new("allow_snapshot_full_refresh", FieldDefault::False),
        FieldSpec::new("allow_table_type_downgrade", FieldDefault::False),
        FieldSpec::new("allow_retention_decrease", FieldDefault::False),
        FieldSpec::new("allow_missing_migration_origin", FieldDefault::False),
        FieldSpec::new("allow_snapshot_schema_change", FieldDefault::False),
        FieldSpec::new("concurrency", FieldDefault::None),
        FieldSpec::new("select", FieldDefault::EmptyTuple),
        FieldSpec::new("exclude", FieldDefault::EmptyTuple),
        FieldSpec::new("verbose", FieldDefault::False),
        FieldSpec::new("debug", FieldDefault::False),
        FieldSpec::new("cli_vars", FieldDefault::None),
        FieldSpec::new("run_tests", FieldDefault::True),
        FieldSpec::new("run_audits", FieldDefault::True),
        FieldSpec::new("manifest", FieldDefault::False),
        FieldSpec::new("json_output", FieldDefault::False),
        FieldSpec::new("json_output_path", FieldDefault::None),
        FieldSpec::new("event_output_path", FieldDefault::None),
        FieldSpec::new("no_cache", FieldDefault::False),
        FieldSpec::new("max_microbatches", FieldDefault::None),
        FieldSpec::new("selection_diagnostics", FieldDefault::False),
        FieldSpec::new("selector_files", FieldDefault::EmptyTuple),
        FieldSpec::new("test_timeout", FieldDefault::None),
        FieldSpec::new("discovered_inputs", FieldDefault::None),
    ],
};
const BUILD_INVOCATION: RecordSpec = RecordSpec {
    name: "BuildInvocation",
    doc: "Resolved project, adapter, and reporter context for the build command.",
    fields: &[
        FieldSpec::required("effective_project_dir"),
        FieldSpec::required("discovered_inputs"),
        FieldSpec::required("effective_defer_clone_from"),
        FieldSpec::required("adapter_name"),
        FieldSpec::required("adapter"),
        FieldSpec::required("connection_config"),
        FieldSpec::required("use_color"),
        FieldSpec::required("progress_stream"),
        FieldSpec::required("connection_progress"),
        FieldSpec::required("planning_progress"),
        FieldSpec::required("should_load_sources"),
        FieldSpec::new("effective_target_name", FieldDefault::None),
        FieldSpec::new(
            "execution_limits",
            FieldDefault::NativeCall(RecordId::ExecutionLimitsConfig),
        ),
    ],
};
const CHECK_COMMAND_REQUEST: RecordSpec = RecordSpec {
    name: "CheckCommandRequest",
    doc: "CLI inputs for one check command invocation.",
    fields: &[
        FieldSpec::new("project_dir", FieldDefault::None),
        FieldSpec::new("no_sql_validation", FieldDefault::False),
        FieldSpec::new("no_color", FieldDefault::False),
        FieldSpec::new("selected_target", FieldDefault::None),
        FieldSpec::new("select", FieldDefault::EmptyTuple),
        FieldSpec::new("exclude", FieldDefault::EmptyTuple),
        FieldSpec::new("cli_vars", FieldDefault::None),
        FieldSpec::new("json_output", FieldDefault::False),
        FieldSpec::new("json_output_path", FieldDefault::None),
    ],
};
const CHECK_INVOCATION: RecordSpec = RecordSpec {
    name: "CheckInvocation",
    doc: "Resolved project, adapter, and reporter context for the check command.",
    fields: &[
        FieldSpec::required("effective_project_dir"),
        FieldSpec::required("discovered_inputs"),
        FieldSpec::required("adapter_name"),
        FieldSpec::required("adapter"),
        FieldSpec::required("connection_config"),
        FieldSpec::required("use_color"),
        FieldSpec::required("progress_stream"),
        FieldSpec::required("connection_progress"),
        FieldSpec::required("planning_progress"),
    ],
};
const CLONE_COMMAND_REQUEST: RecordSpec = RecordSpec {
    name: "CloneCommandRequest",
    doc: "CLI inputs for one clone command invocation.",
    fields: &[
        FieldSpec::required("project_dir"),
        FieldSpec::required("no_color"),
        FieldSpec::required("no_sql_validation"),
        FieldSpec::required("origin_target_name"),
        FieldSpec::required("destination_target_name"),
        FieldSpec::required("hard_copy"),
        FieldSpec::new("select", FieldDefault::EmptyTuple),
        FieldSpec::new("exclude", FieldDefault::EmptyTuple),
        FieldSpec::new("verbose", FieldDefault::False),
        FieldSpec::new("cli_vars", FieldDefault::None),
        FieldSpec::new("json_output_path", FieldDefault::None),
        FieldSpec::new("event_output_path", FieldDefault::None),
        FieldSpec::new("discovered_inputs", FieldDefault::None),
    ],
};
const DBT_SQLBUILD_WORK_CONTEXT: RecordSpec = RecordSpec {
    name: "DbtSqlbuildWorkContext",
    doc: "Shared execution context for one dbt interop SQLBuild work run.",
    fields: &[
        FieldSpec::required("plan_output"),
        FieldSpec::required("connection_config"),
        FieldSpec::required("adapter"),
        FieldSpec::required("adapter_name"),
        FieldSpec::required("output_stream"),
        FieldSpec::required("use_color"),
        FieldSpec::required("snapshots_config"),
        FieldSpec::required("execution_limits"),
        FieldSpec::required("concurrency"),
    ],
};
const PLAN_SAFETY_GATES: RecordSpec = RecordSpec {
    name: "PlanSafetyGates",
    doc: "Pre-confirmed plan safety gates and the guidance shown when one needs a terminal.",
    fields: &[
        FieldSpec::required("allow_missing_migration_origin"),
        FieldSpec::required("allow_snapshot_full_refresh"),
        FieldSpec::required("allow_table_type_downgrade"),
        FieldSpec::required("allow_retention_decrease"),
        FieldSpec::required("missing_origin_help"),
        FieldSpec::required("snapshot_full_refresh_help"),
        FieldSpec::required("table_type_downgrade_help"),
        FieldSpec::required("retention_decrease_help"),
        FieldSpec::required("execution_limit_note"),
    ],
};
const DBT_INIT_COMMAND_REQUEST: RecordSpec = RecordSpec {
    name: "DbtInitCommandRequest",
    doc: "CLI inputs for one dbt init command invocation.",
    fields: &[
        FieldSpec::required("cwd"),
        FieldSpec::required("dbt_project_dir"),
        FieldSpec::required("profiles_dir"),
        FieldSpec::required("profile_name"),
        FieldSpec::required("target_name"),
        FieldSpec::required("sqb_output_dir"),
        FieldSpec::required("dry_run"),
        FieldSpec::required("overwrite"),
        FieldSpec::required("skip_dbt_debug"),
    ],
};
const DIFF_COMMAND_REQUEST: RecordSpec = RecordSpec {
    name: "DiffCommandRequest",
    doc: "CLI inputs for one diff command invocation.",
    fields: &[
        FieldSpec::required("project_dir"),
        FieldSpec::required("no_color"),
        FieldSpec::required("no_sql_validation"),
        FieldSpec::required("from_name"),
        FieldSpec::required("to_name"),
        FieldSpec::required("full"),
        FieldSpec::required("schema_only"),
        FieldSpec::required("bounded"),
        FieldSpec::new("max_column_examples", FieldDefault::None),
        FieldSpec::new("max_row_only_examples", FieldDefault::None),
        FieldSpec::new("sample_rows", FieldDefault::None),
        FieldSpec::new("sample_seed", FieldDefault::None),
        FieldSpec::new("exhaustive", FieldDefault::False),
        FieldSpec::new("json_output", FieldDefault::False),
        FieldSpec::new("json_output_path", FieldDefault::None),
        FieldSpec::new("max_models", FieldDefault::None),
        FieldSpec::new("max_columns", FieldDefault::None),
        FieldSpec::new("select", FieldDefault::EmptyTuple),
        FieldSpec::new("exclude", FieldDefault::EmptyTuple),
        FieldSpec::new("verbose", FieldDefault::False),
        FieldSpec::new("cli_vars", FieldDefault::None),
        FieldSpec::new("selected_target", FieldDefault::None),
        FieldSpec::new("left_query", FieldDefault::None),
        FieldSpec::new("left_query_file", FieldDefault::None),
        FieldSpec::new("left_label", FieldDefault::None),
        FieldSpec::new("right_query", FieldDefault::None),
        FieldSpec::new("right_query_file", FieldDefault::None),
        FieldSpec::new("right_label", FieldDefault::None),
        FieldSpec::new("unique_key_override", FieldDefault::EmptyTuple),
        FieldSpec::new("unkeyed", FieldDefault::False),
        FieldSpec::new("excluded_columns_override", FieldDefault::EmptyTuple),
        FieldSpec::new("tolerance_overrides", FieldDefault::EmptyTuple),
        FieldSpec::new("max_value_length", FieldDefault::None),
        FieldSpec::new("suppress_example_values", FieldDefault::False),
        FieldSpec::new("full_example_values", FieldDefault::False),
    ],
};
const REVIEW_COMMAND_REQUEST: RecordSpec = RecordSpec {
    name: "ReviewCommandRequest",
    doc: "CLI inputs for one `sqb review` invocation.",
    fields: &[
        FieldSpec::new("project_dir", FieldDefault::None),
        FieldSpec::new("no_color", FieldDefault::False),
        FieldSpec::new("no_sql_validation", FieldDefault::False),
        FieldSpec::new("selected_target", FieldDefault::None),
        FieldSpec::new("select", FieldDefault::EmptyTuple),
        FieldSpec::new("cli_vars", FieldDefault::None),
        FieldSpec::new("json_output", FieldDefault::False),
        FieldSpec::new("since", FieldDefault::None),
        FieldSpec::new("against", FieldDefault::None),
        FieldSpec::new("downstream", FieldDefault::None),
        FieldSpec::new("dry_run", FieldDefault::False),
        FieldSpec::new("no_clone", FieldDefault::False),
        FieldSpec::new("bounded", FieldDefault::None),
        FieldSpec::new("markdown_path", FieldDefault::None),
        FieldSpec::new("concurrency", FieldDefault::None),
        FieldSpec::new("verbose", FieldDefault::False),
        FieldSpec::new("debug", FieldDefault::False),
    ],
};
const QUERY_DIFF_PREPARATION: RecordSpec = RecordSpec {
    name: "QueryDiffPreparation",
    doc: "Resolved raw-query diff adapter, connection, SQL, and limits.",
    fields: &[
        FieldSpec::required("adapter"),
        FieldSpec::required("connection_config"),
        FieldSpec::required("left_sql"),
        FieldSpec::required("right_sql"),
        FieldSpec::required("left_label"),
        FieldSpec::required("right_label"),
        FieldSpec::required("selected_target"),
        FieldSpec::required("database"),
        FieldSpec::required("schema"),
        FieldSpec::required("run_id"),
        FieldSpec::required("artifact_ttl"),
        FieldSpec::required("effective_max_column_examples"),
        FieldSpec::required("effective_max_row_only_examples"),
    ],
};
const FRESHNESS_COMMAND_REQUEST: RecordSpec = RecordSpec {
    name: "FreshnessCommandRequest",
    doc: "CLI inputs for one `sqb freshness` invocation.",
    fields: &[
        FieldSpec::new("project_dir", FieldDefault::None),
        FieldSpec::new("no_sql_validation", FieldDefault::False),
        FieldSpec::new("no_color", FieldDefault::False),
        FieldSpec::new("selected_target", FieldDefault::None),
        FieldSpec::new("select", FieldDefault::EmptyTuple),
        FieldSpec::new("exclude", FieldDefault::EmptyTuple),
        FieldSpec::new("cli_vars", FieldDefault::None),
        FieldSpec::new("json_output", FieldDefault::False),
        FieldSpec::new("json_output_path", FieldDefault::None),
        FieldSpec::new("fail_on_error", FieldDefault::False),
        FieldSpec::new("compare_state", FieldDefault::False),
        FieldSpec::new("fail_on_stale", FieldDefault::False),
    ],
};
const JANITOR_COMMAND_REQUEST: RecordSpec = RecordSpec {
    name: "JanitorCommandRequest",
    doc: "CLI inputs for one janitor command invocation.",
    fields: &[
        FieldSpec::required("project_dir"),
        FieldSpec::new("no_color", FieldDefault::False),
        FieldSpec::new("auto_approve", FieldDefault::False),
        FieldSpec::new("retention_days", FieldDefault::None),
        FieldSpec::new("direct_state_history_versions", FieldDefault::None),
        FieldSpec::new("drop_old_name_views", FieldDefault::EmptyTuple),
        FieldSpec::new("keep_in_use", FieldDefault::False),
        FieldSpec::new("selected_target", FieldDefault::None),
        FieldSpec::new("as_target", FieldDefault::None),
        FieldSpec::new("verbose", FieldDefault::False),
        FieldSpec::new("json_output", FieldDefault::False),
    ],
};
const JANITOR_INVOCATION: RecordSpec = RecordSpec {
    name: "JanitorInvocation",
    doc: "Resolved project discovery and output context for janitor.",
    fields: &[
        FieldSpec::required("effective_project_dir"),
        FieldSpec::required("discovered_inputs"),
        FieldSpec::required("use_color"),
        FieldSpec::new("selected_target", FieldDefault::None),
        FieldSpec::new("active_target_name", FieldDefault::None),
        FieldSpec::new("as_target", FieldDefault::None),
        FieldSpec::new("machine_output", FieldDefault::False),
    ],
};
const JANITOR_SETTINGS: RecordSpec = RecordSpec {
    name: "JanitorSettings",
    doc: "Validated effective janitor settings.",
    fields: &[
        FieldSpec::required("retention_days"),
        FieldSpec::required("direct_state_history_versions"),
        FieldSpec::new("archive_retention_days", FieldDefault::Int(14)),
        FieldSpec::new("drop_old_name_views", FieldDefault::EmptyTuple),
        FieldSpec::new("in_use_window", FieldDefault::Text("7d")),
        FieldSpec::new("keep_in_use", FieldDefault::False),
    ],
};
const LOAD_COMMAND_REQUEST: RecordSpec = RecordSpec {
    name: "LoadCommandRequest",
    doc: "CLI inputs for one load command invocation.",
    fields: &[
        FieldSpec::required("project_dir"),
        FieldSpec::new("no_color", FieldDefault::False),
        FieldSpec::new("selected_target", FieldDefault::None),
        FieldSpec::new("select", FieldDefault::EmptyTuple),
        FieldSpec::new("exclude", FieldDefault::EmptyTuple),
        FieldSpec::new("reload", FieldDefault::False),
        FieldSpec::new("concurrency", FieldDefault::None),
        FieldSpec::new("cursor_overrides", FieldDefault::None),
        FieldSpec::new("cli_vars", FieldDefault::None),
        FieldSpec::new("json_output", FieldDefault::False),
        FieldSpec::new("json_output_path", FieldDefault::None),
    ],
};
const PLAN_COMMAND_REQUEST: RecordSpec = RecordSpec {
    name: "PlanCommandRequest",
    doc: "CLI inputs for one plan command invocation.",
    fields: &[
        FieldSpec::new("project_dir", FieldDefault::None),
        FieldSpec::new("no_sql_validation", FieldDefault::False),
        FieldSpec::new("defer_to", FieldDefault::None),
        FieldSpec::new("defer_sources_to", FieldDefault::None),
        FieldSpec::new("selected_target", FieldDefault::None),
        FieldSpec::new("as_target", FieldDefault::None),
        FieldSpec::new("cursor_overrides", FieldDefault::None),
        FieldSpec::new("json_output", FieldDefault::False),
        FieldSpec::new("full_refresh", FieldDefault::False),
        FieldSpec::new("load_sources", FieldDefault::None),
        FieldSpec::new("include_python", FieldDefault::True),
        FieldSpec::new("no_color", FieldDefault::False),
        FieldSpec::new("select", FieldDefault::EmptyTuple),
        FieldSpec::new("exclude", FieldDefault::EmptyTuple),
        FieldSpec::new("verbose", FieldDefault::False),
        FieldSpec::new("cli_vars", FieldDefault::None),
        FieldSpec::new("no_cache", FieldDefault::False),
        FieldSpec::new("max_microbatches", FieldDefault::None),
        FieldSpec::new("selection_diagnostics", FieldDefault::False),
    ],
};
const PLAYGROUND_COMMAND_REQUEST: RecordSpec = RecordSpec {
    name: "PlaygroundCommandRequest",
    doc: "CLI inputs for one playground command invocation.",
    fields: &[
        FieldSpec::required("project_dir"),
        FieldSpec::required("target_path"),
        FieldSpec::new("template", FieldDefault::Text("waffle_shop")),
    ],
};
const ADAPTER_CONNECTION_CONTEXT: RecordSpec = RecordSpec {
    name: "AdapterConnectionContext",
    doc: "Resolved adapter and connection configuration for one CLI command.",
    fields: &[
        FieldSpec::required("adapter_name"),
        FieldSpec::required("adapter"),
        FieldSpec::required("connection_config"),
    ],
};
const SCENARIO_SNAPSHOT_LIMIT_INPUTS: RecordSpec = RecordSpec {
    name: "ScenarioSnapshotLimitInputs",
    doc: "CLI snapshot capture-limit overrides for one scenario run.",
    fields: &[
        FieldSpec::new("max_snapshot_rows", FieldDefault::None),
        FieldSpec::new("max_snapshot_total_rows", FieldDefault::None),
        FieldSpec::new("max_snapshot_bytes", FieldDefault::None),
        FieldSpec::new("max_snapshot_total_bytes", FieldDefault::None),
        FieldSpec::new("force", FieldDefault::False),
    ],
};
const SCENARIO_TEST_COMMAND_REQUEST: RecordSpec = RecordSpec {
    name: "ScenarioTestCommandRequest",
    doc: "CLI inputs for one `sqb scenario test` invocation.",
    fields: &[
        FieldSpec::new("project_dir", FieldDefault::None),
        FieldSpec::new("scenario_namespace", FieldDefault::None),
        FieldSpec::new("no_sql_validation", FieldDefault::False),
        FieldSpec::new("no_color", FieldDefault::False),
        FieldSpec::new("selectors", FieldDefault::EmptyTuple),
        FieldSpec::new("exclude", FieldDefault::EmptyTuple),
        FieldSpec::new("retain", FieldDefault::False),
        FieldSpec::new("local", FieldDefault::False),
        FieldSpec::new("strict", FieldDefault::False),
        FieldSpec::new("concurrency", FieldDefault::None),
        FieldSpec::new("sync_snapshots", FieldDefault::False),
        FieldSpec::new("refresh", FieldDefault::False),
        FieldSpec::new(
            "limit_inputs",
            FieldDefault::NativeCall(RecordId::ScenarioSnapshotLimitInputs),
        ),
        FieldSpec::new("json_output", FieldDefault::False),
        FieldSpec::new("json_output_path", FieldDefault::None),
        FieldSpec::new("scenario_timeout", FieldDefault::None),
        FieldSpec::new("since", FieldDefault::None),
        FieldSpec::new("explain", FieldDefault::False),
    ],
};
const SCENARIO_CAPTURE_COMMAND_REQUEST: RecordSpec = RecordSpec {
    name: "ScenarioCaptureCommandRequest",
    doc: "CLI inputs for one `sqb scenario capture` invocation.",
    fields: &[
        FieldSpec::new("project_dir", FieldDefault::None),
        FieldSpec::new("scenario_namespace", FieldDefault::None),
        FieldSpec::new("no_sql_validation", FieldDefault::False),
        FieldSpec::new("no_color", FieldDefault::False),
        FieldSpec::new("selectors", FieldDefault::EmptyTuple),
        FieldSpec::new("exclude", FieldDefault::EmptyTuple),
        FieldSpec::new("retain", FieldDefault::False),
        FieldSpec::new(
            "limit_inputs",
            FieldDefault::NativeCall(RecordId::ScenarioSnapshotLimitInputs),
        ),
    ],
};
const SEED_COMMAND_REQUEST: RecordSpec = RecordSpec {
    name: "SeedCommandRequest",
    doc: "CLI inputs for one seed command invocation.",
    fields: &[
        FieldSpec::required("project_dir"),
        FieldSpec::new("no_color", FieldDefault::False),
        FieldSpec::new("selected_target", FieldDefault::None),
        FieldSpec::new("select", FieldDefault::EmptyTuple),
        FieldSpec::new("exclude", FieldDefault::EmptyTuple),
        FieldSpec::new("concurrency", FieldDefault::None),
        FieldSpec::new("cli_vars", FieldDefault::None),
        FieldSpec::new("json_output", FieldDefault::False),
        FieldSpec::new("json_output_path", FieldDefault::None),
    ],
};
const SEED_INVOCATION: RecordSpec = RecordSpec {
    name: "SeedInvocation",
    doc: "Resolved project, adapter, connection, and output context for seed.",
    fields: &[
        FieldSpec::required("effective_project_dir"),
        FieldSpec::required("discovered_inputs"),
        FieldSpec::required("adapter_name"),
        FieldSpec::required("adapter"),
        FieldSpec::required("connection_config"),
        FieldSpec::required("use_color"),
        FieldSpec::required("progress_stream"),
    ],
};
const TEST_COMMAND_REQUEST: RecordSpec = RecordSpec {
    name: "TestCommandRequest",
    doc: "CLI inputs for one test command invocation.",
    fields: &[
        FieldSpec::new("project_dir", FieldDefault::None),
        FieldSpec::new("no_sql_validation", FieldDefault::False),
        FieldSpec::new("no_color", FieldDefault::False),
        FieldSpec::new("selected_target", FieldDefault::None),
        FieldSpec::new("select", FieldDefault::EmptyTuple),
        FieldSpec::new("exclude", FieldDefault::EmptyTuple),
        FieldSpec::new("cli_vars", FieldDefault::None),
        FieldSpec::new("json_output", FieldDefault::False),
        FieldSpec::new("json_output_path", FieldDefault::None),
        FieldSpec::new("concurrency", FieldDefault::None),
        FieldSpec::new("case_name", FieldDefault::None),
        FieldSpec::new("inspect", FieldDefault::False),
        FieldSpec::new("test_timeout", FieldDefault::None),
        FieldSpec::new("since", FieldDefault::None),
        FieldSpec::new("explain", FieldDefault::False),
    ],
};
const REFACTOR_COMMAND_REQUEST: RecordSpec = RecordSpec {
    name: "RefactorCommandRequest",
    doc: "CLI inputs for one `sqb rename` or `sqb mv` invocation.",
    fields: &[
        FieldSpec::required("command"),
        FieldSpec::new("target", FieldDefault::None),
        FieldSpec::new("new_name", FieldDefault::None),
        FieldSpec::new("destination", FieldDefault::None),
        FieldSpec::new("more_moves", FieldDefault::EmptyTuple),
        FieldSpec::new("from_file", FieldDefault::None),
        FieldSpec::new("project_dir", FieldDefault::None),
        FieldSpec::new("cascade", FieldDefault::False),
        FieldSpec::new("dry_run", FieldDefault::False),
        FieldSpec::new("json_output", FieldDefault::False),
        FieldSpec::new("no_color", FieldDefault::False),
    ],
};
const SCOPE_COMMAND_REQUEST: RecordSpec = RecordSpec {
    name: "ScopeCommandRequest",
    doc: "CLI inputs for one offline declaration-scope inspection.",
    fields: &[
        FieldSpec::new("project_dir", FieldDefault::None),
        FieldSpec::new("target", FieldDefault::None),
        FieldSpec::new("at", FieldDefault::None),
        FieldSpec::new("as_path", FieldDefault::None),
        FieldSpec::new("browse", FieldDefault::None),
        FieldSpec::new("list_path", FieldDefault::None),
        FieldSpec::new("defined_under", FieldDefault::None),
        FieldSpec::new("kinds", FieldDefault::EmptyTuple),
        FieldSpec::new("match", FieldDefault::None),
        FieldSpec::new("used_only", FieldDefault::False),
        FieldSpec::new("include_nearby", FieldDefault::False),
        FieldSpec::new("nearby_depth", FieldDefault::Int(1)),
        FieldSpec::new("dependency_depth", FieldDefault::Int(0)),
        FieldSpec::new("explain", FieldDefault::None),
        FieldSpec::new("globals", FieldDefault::Text("summary")),
        FieldSpec::new("page_size", FieldDefault::Int(100)),
        FieldSpec::new("after", FieldDefault::None),
        FieldSpec::new("paths", FieldDefault::Text("relative")),
        FieldSpec::new("json_output", FieldDefault::False),
        FieldSpec::new("no_cache", FieldDefault::False),
        FieldSpec::new("verbose", FieldDefault::False),
        FieldSpec::new("no_color", FieldDefault::False),
    ],
};
const TEST_INVOCATION: RecordSpec = RecordSpec {
    name: "TestInvocation",
    doc: "Resolved project, adapter, and reporter context for the test command.",
    fields: &[
        FieldSpec::required("effective_project_dir"),
        FieldSpec::required("discovered_inputs"),
        FieldSpec::required("adapter_name"),
        FieldSpec::required("adapter"),
        FieldSpec::required("connection_config"),
        FieldSpec::required("use_color"),
        FieldSpec::required("progress_stream"),
        FieldSpec::required("connection_progress"),
        FieldSpec::required("planning_progress"),
    ],
};
const RULES_COMMAND_REQUEST: RecordSpec = RecordSpec {
    name: "RulesCommandRequest",
    doc: "Inputs for focused rule execution, inspection, or skill generation.",
    fields: &[
        FieldSpec::required("project_dir"),
        FieldSpec::required("json_output"),
        FieldSpec::required("action"),
        FieldSpec::required("rule_selector"),
        FieldSpec::required("skills_check"),
        FieldSpec::new("select", FieldDefault::EmptyTuple),
        FieldSpec::new("exclude", FieldDefault::EmptyTuple),
    ],
};
const COST_COMMAND_REQUEST: RecordSpec = RecordSpec {
    name: "CostCommandRequest",
    doc: "Inputs for local run-cost history and drill-down.",
    fields: &[
        FieldSpec::required("project_dir"),
        FieldSpec::required("selector"),
        FieldSpec::required("no_color"),
        FieldSpec::required("limit"),
        FieldSpec::required("no_limit"),
        FieldSpec::required("sort"),
        FieldSpec::required("order"),
        FieldSpec::required("since"),
        FieldSpec::required("until"),
        FieldSpec::required("json_output"),
        FieldSpec::required("json_output_path"),
    ],
};
const BUILD_COST_FINALIZATION: RecordSpec = RecordSpec {
    name: "BuildCostFinalization",
    doc: "Inputs for nonfatal automatic build-cost finalization.",
    fields: &[
        FieldSpec::required("project_dir"),
        FieldSpec::required("adapter_name"),
        FieldSpec::required("adapter"),
        FieldSpec::required("connection_config"),
        FieldSpec::required("target_name"),
        FieldSpec::required("target_database"),
        FieldSpec::required("run_id"),
        FieldSpec::required("build_status"),
        FieldSpec::required("started_at"),
        FieldSpec::required("completed_at"),
        FieldSpec::required("config"),
        FieldSpec::required("output_stream"),
        FieldSpec::required("use_color"),
        FieldSpec::new("collect", FieldDefault::True),
        FieldSpec::new("render", FieldDefault::True),
        FieldSpec::new(
            "cost_status",
            FieldDefault::HostMember(ExecutorHost::CostStatus.id(), "COMPLETE"),
        ),
        FieldSpec::new(
            "cost_message",
            FieldDefault::Text("No executable warehouse work was planned for this build."),
        ),
        FieldSpec::new("had_executable_work", FieldDefault::True),
    ],
};
const CONTRACT_COMMAND_REQUEST: RecordSpec = RecordSpec {
    name: "ContractCommandRequest",
    doc: "CLI inputs for physical contract comparison and repository adoption.",
    fields: &[
        FieldSpec::required("action"),
        FieldSpec::required("from_target"),
        FieldSpec::new("project_dir", FieldDefault::None),
        FieldSpec::new("select", FieldDefault::EmptyTuple),
        FieldSpec::new("exclude", FieldDefault::EmptyTuple),
        FieldSpec::new("write", FieldDefault::False),
        FieldSpec::new("overwrite", FieldDefault::False),
        FieldSpec::new("json_output", FieldDefault::False),
        FieldSpec::new("no_color", FieldDefault::False),
        FieldSpec::new("cli_vars", FieldDefault::None),
    ],
};
const LINEAGE_COMMAND_REQUEST: RecordSpec = RecordSpec {
    name: "LineageCommandRequest",
    doc: "CLI inputs for dependency and column lineage inspection.",
    fields: &[
        FieldSpec::required("project_dir"),
        FieldSpec::new("no_sql_validation", FieldDefault::False),
        FieldSpec::new("targets", FieldDefault::EmptyTuple),
        FieldSpec::new("output_format", FieldDefault::Text("tree")),
        FieldSpec::new("direction", FieldDefault::None),
        FieldSpec::new("depth", FieldDefault::Text("all")),
        FieldSpec::new("select", FieldDefault::EmptyTuple),
        FieldSpec::new("exclude", FieldDefault::EmptyTuple),
        FieldSpec::new(
            "lineage_mode",
            FieldDefault::NativeMember(RecordId::ColumnLineageMode, "RICH"),
        ),
        FieldSpec::new("include_uses", FieldDefault::False),
        FieldSpec::new("cli_vars", FieldDefault::None),
    ],
};

/// `sqb` command requests and their resolved invocation contexts.
const COMMAND_RECORDS: &[RecordSpec] = &[
    NESTED_PROGRESS_CHILD_ROW,
    AUDIT_COMMAND_REQUEST,
    AUDIT_INVOCATION,
    BUILD_COMMAND_REQUEST,
    BUILD_INVOCATION,
    CHECK_COMMAND_REQUEST,
    CHECK_INVOCATION,
    CLONE_COMMAND_REQUEST,
    DBT_SQLBUILD_WORK_CONTEXT,
    PLAN_SAFETY_GATES,
    DBT_INIT_COMMAND_REQUEST,
    DIFF_COMMAND_REQUEST,
    REVIEW_COMMAND_REQUEST,
    QUERY_DIFF_PREPARATION,
    FRESHNESS_COMMAND_REQUEST,
    JANITOR_COMMAND_REQUEST,
    JANITOR_INVOCATION,
    JANITOR_SETTINGS,
    LOAD_COMMAND_REQUEST,
    PLAN_COMMAND_REQUEST,
    PLAYGROUND_COMMAND_REQUEST,
    ADAPTER_CONNECTION_CONTEXT,
    SCENARIO_SNAPSHOT_LIMIT_INPUTS,
    SCENARIO_TEST_COMMAND_REQUEST,
    SCENARIO_CAPTURE_COMMAND_REQUEST,
    SEED_COMMAND_REQUEST,
    SEED_INVOCATION,
    TEST_COMMAND_REQUEST,
    REFACTOR_COMMAND_REQUEST,
    SCOPE_COMMAND_REQUEST,
    TEST_INVOCATION,
    RULES_COMMAND_REQUEST,
    COST_COMMAND_REQUEST,
    BUILD_COST_FINALIZATION,
    CONTRACT_COMMAND_REQUEST,
    LINEAGE_COMMAND_REQUEST,
];

const COMPILE_PROFILE_FLAGS: RecordSpec = RecordSpec {
    name: "CompileProfileFlags",
    doc: "Profiling toggles that skip compile phases for benchmarking.",
    fields: &[
        FieldSpec::new("skip_discovery_sql_analysis", FieldDefault::False),
        FieldSpec::new("skip_column_inference", FieldDefault::False),
        FieldSpec::new("skip_contracts", FieldDefault::False),
        FieldSpec::new("skip_write", FieldDefault::False),
    ],
};
const COMPILE_COMMAND_REQUEST: RecordSpec = RecordSpec {
    name: "CompileCommandRequest",
    doc: "CLI inputs for one `sqb compile` invocation.",
    fields: &[
        FieldSpec::new("project_dir", FieldDefault::None),
        FieldSpec::new("no_sql_validation", FieldDefault::False),
        FieldSpec::new("defer_to", FieldDefault::None),
        FieldSpec::new("selected_target", FieldDefault::None),
        FieldSpec::new("json_output", FieldDefault::False),
        FieldSpec::new("manifest", FieldDefault::False),
        FieldSpec::new("dag_path", FieldDefault::None),
        FieldSpec::new("no_color", FieldDefault::False),
        FieldSpec::new(
            "lineage_mode",
            FieldDefault::NativeMember(RecordId::CompileLineageMode, "FAST"),
        ),
        FieldSpec::new("select", FieldDefault::EmptyTuple),
        FieldSpec::new("exclude", FieldDefault::EmptyTuple),
        FieldSpec::new("cli_vars", FieldDefault::None),
        FieldSpec::new(
            "profile_flags",
            FieldDefault::NativeCall(RecordId::CompileProfileFlags),
        ),
        FieldSpec::new("no_cache", FieldDefault::False),
        FieldSpec::new("debug", FieldDefault::False),
    ],
};
const SELECTOR_FILE_SUMMARY: RecordSpec = RecordSpec {
    name: "SelectorFileSummary",
    doc: "Non-expanded provenance for one selector file.",
    fields: &[
        FieldSpec::required("path"),
        FieldSpec::required("selector_count"),
    ],
};
const PARSED_CLI_INVOCATION: RecordSpec = RecordSpec {
    name: "ParsedCliInvocation",
    doc: "Outcome of parsing CLI arguments: either a namespace or an exit code.",
    fields: &[
        FieldSpec::required("args"),
        FieldSpec::required("exit_code"),
    ],
};
const CLI_ENTRYPOINT_HANDLERS: RecordSpec = RecordSpec {
    name: "CliEntrypointHandlers",
    doc: "Injected command handlers for the CLI entrypoint.",
    fields: &[
        FieldSpec::required("run_compile"),
        FieldSpec::required("run_cost"),
        FieldSpec::required("run_dag"),
        FieldSpec::required("run_plan"),
        FieldSpec::required("run_dbt_plan"),
        FieldSpec::required("run_dbt_run"),
        FieldSpec::required("run_dbt_build"),
        FieldSpec::required("run_dbt_debug"),
        FieldSpec::required("run_dbt_init"),
        FieldSpec::required("run_build"),
        FieldSpec::required("run_freshness"),
        FieldSpec::required("run_test"),
        FieldSpec::required("run_check"),
        FieldSpec::required("run_audit"),
        FieldSpec::required("run_seed"),
        FieldSpec::required("run_load"),
        FieldSpec::required("run_clone"),
        FieldSpec::required("run_diff"),
        FieldSpec::required("run_query"),
        FieldSpec::required("run_debug"),
        FieldSpec::required("run_lineage"),
        FieldSpec::required("run_janitor"),
        FieldSpec::required("run_init"),
        FieldSpec::required("run_playground"),
        FieldSpec::required("run_skills_update"),
        FieldSpec::required("run_format"),
        FieldSpec::required("run_scenario"),
        FieldSpec::required("run_scenario_capture"),
        FieldSpec::required("run_rules"),
        FieldSpec::required("run_scope"),
        FieldSpec::new("run_contract", FieldDefault::None),
        FieldSpec::new("run_refactor", FieldDefault::None),
        FieldSpec::new("run_recall", FieldDefault::None),
        FieldSpec::new("run_review", FieldDefault::None),
        FieldSpec::new("run_location", FieldDefault::None),
    ],
};
const COMMAND_WAREHOUSE_SCOPE: RecordSpec = RecordSpec {
    name: "CommandWarehouseScope",
    doc: "The warehouse group and CLI override active for one command invocation.",
    fields: &[
        FieldSpec::required("group"),
        FieldSpec::new("cli_warehouse", FieldDefault::None),
        FieldSpec::new("cli_vars", FieldDefault::EmptyDict),
    ],
};
const RESOLVED_WAREHOUSE: RecordSpec = RecordSpec {
    name: "ResolvedWarehouse",
    doc: "The warehouse one command connects with and where that value came from.",
    fields: &[
        FieldSpec::required("warehouse"),
        FieldSpec::required("source"),
    ],
};

/// Parsed `sqb` invocations, compile requests, entry handlers and command warehouses.
const ENTRY_RECORDS: &[RecordSpec] = &[
    COMPILE_PROFILE_FLAGS,
    COMPILE_COMMAND_REQUEST,
    SELECTOR_FILE_SUMMARY,
    PARSED_CLI_INVOCATION,
    CLI_ENTRYPOINT_HANDLERS,
    COMMAND_WAREHOUSE_SCOPE,
    RESOLVED_WAREHOUSE,
];

const WRITTEN_TARGET: RecordSpec = RecordSpec {
    name: "WrittenTarget",
    doc: "Result of writing compiled output to target/.",
    fields: &[
        FieldSpec::required("model_count"),
        FieldSpec::required("seed_count"),
        FieldSpec::required("function_count"),
        FieldSpec::required("audit_count"),
        FieldSpec::required("test_count"),
        FieldSpec::required("target_dir"),
        FieldSpec::new("diagnostics", FieldDefault::EmptyTuple),
        FieldSpec::new("failed_compile", FieldDefault::False),
        FieldSpec::new("failed_model_sql", FieldDefault::EmptyTuple),
    ],
};
const SKILL_MAINTENANCE_RESULT: RecordSpec = RecordSpec {
    name: "SkillMaintenanceResult",
    doc: "Non-blocking generated-skill maintenance result.",
    fields: &[FieldSpec::new("message", FieldDefault::Text(""))],
};
const SKILL_UPDATE_RESULT: RecordSpec = RecordSpec {
    name: "SkillUpdateResult",
    doc: "Result of installing or updating skill files.",
    fields: &[FieldSpec::required("written_paths")],
};

/// Compile output and skill maintenance results.
const OUTPUT_RECORDS: &[RecordSpec] = &[
    WRITTEN_TARGET,
    SKILL_MAINTENANCE_RESULT,
    SKILL_UPDATE_RESULT,
];

const COMPILE_ANALYSIS: RecordSpec = RecordSpec {
    name: "CompileAnalysis",
    doc: "Compiled project analysis shared by compile output phases.",
    fields: &[
        FieldSpec::required("discovered_inputs"),
        FieldSpec::required("adapter"),
        FieldSpec::required("graph"),
        FieldSpec::required("selected_keys"),
        FieldSpec::required("lineage"),
        FieldSpec::required("diagnostics"),
        FieldSpec::required("discover_ms"),
        FieldSpec::required("graph_ms"),
        FieldSpec::required("lineage_ms"),
        FieldSpec::required("contract_ms"),
        FieldSpec::required("contract_cpu_ms"),
        FieldSpec::new("built_in_rules_ms", FieldDefault::Int(0)),
        FieldSpec::new("custom_rules_ms", FieldDefault::Int(0)),
        FieldSpec::new("custom_rules_cpu_ms", FieldDefault::Int(0)),
        FieldSpec::new("early_lint_wait_ms", FieldDefault::Int(0)),
        FieldSpec::new("rule_cache_hits", FieldDefault::Int(0)),
        FieldSpec::new("rule_cache_misses", FieldDefault::Int(0)),
        FieldSpec::new("skipped_type_proof_rules", FieldDefault::EmptyTuple),
        FieldSpec::new("custom_rule_timings", FieldDefault::EmptyTuple),
    ],
};

/// The compiled project analysis shared by compile output phases.
const COMPILE_RECORDS: &[RecordSpec] = &[COMPILE_ANALYSIS];

const DBT_MANIFEST_MODEL: RecordSpec = RecordSpec {
    name: "DbtManifestModel",
    doc: "One dbt model node needed for SQLBuild dbt_ref resolution.",
    fields: &[
        FieldSpec::required("unique_id"),
        FieldSpec::required("package_name"),
        FieldSpec::required("name"),
        FieldSpec::required("relation_name"),
        FieldSpec::new("database", FieldDefault::None),
        FieldSpec::new("schema", FieldDefault::None),
        FieldSpec::new("alias", FieldDefault::None),
        FieldSpec::new("node_checksum", FieldDefault::None),
        FieldSpec::new("fqn", FieldDefault::EmptyTuple),
        FieldSpec::new("query_sql", FieldDefault::Text("")),
        FieldSpec::new("depends_on_nodes", FieldDefault::EmptyTuple),
        FieldSpec::new("payload", FieldDefault::EmptyDict),
    ],
};
const DBT_MANIFEST_SOURCE: RecordSpec = RecordSpec {
    name: "DbtManifestSource",
    doc: "One dbt source node needed for graph and lineage resolution.",
    fields: &[
        FieldSpec::required("unique_id"),
        FieldSpec::required("package_name"),
        FieldSpec::required("source_name"),
        FieldSpec::required("name"),
        FieldSpec::required("relation_name"),
        FieldSpec::new("database", FieldDefault::None),
        FieldSpec::new("schema", FieldDefault::None),
        FieldSpec::new("identifier", FieldDefault::None),
        FieldSpec::new("payload", FieldDefault::EmptyDict),
    ],
};
const DBT_MANIFEST_SEED: RecordSpec = RecordSpec {
    name: "DbtManifestSeed",
    doc: "One dbt seed node, a leaf data relation used for column lineage.",
    fields: &[
        FieldSpec::required("unique_id"),
        FieldSpec::required("package_name"),
        FieldSpec::required("name"),
        FieldSpec::required("relation_name"),
        FieldSpec::new("database", FieldDefault::None),
        FieldSpec::new("schema", FieldDefault::None),
        FieldSpec::new("alias", FieldDefault::None),
        FieldSpec::new("payload", FieldDefault::EmptyDict),
    ],
};
const DBT_MANIFEST_INDEX: RecordSpec = RecordSpec {
    name: "DbtManifestIndex",
    doc: "Lookup indexes for dbt model and source nodes in a manifest.",
    fields: &[
        FieldSpec::required("models_by_unique_id"),
        FieldSpec::required("models_by_name"),
        FieldSpec::required("models_by_package_and_name"),
        FieldSpec::new("sources_by_unique_id", FieldDefault::EmptyDict),
        FieldSpec::new("seeds_by_unique_id", FieldDefault::EmptyDict),
        FieldSpec::new(
            "validation_depends_on_nodes_by_unique_id",
            FieldDefault::EmptyDict,
        ),
    ],
};
const DBT_CLI_CONFIG_OVERRIDES: RecordSpec = RecordSpec {
    name: "DbtCliConfigOverrides",
    doc: "dbt config values supplied by CLI flags.",
    fields: &[
        FieldSpec::new("project_dir", FieldDefault::None),
        FieldSpec::new("profiles_dir", FieldDefault::None),
        FieldSpec::new("target", FieldDefault::None),
        FieldSpec::new("target_path", FieldDefault::None),
    ],
};
const RESOLVED_DBT_CONFIG: RecordSpec = RecordSpec {
    name: "ResolvedDbtConfig",
    doc: "Resolved dbt config after applying CLI overrides.",
    fields: &[
        FieldSpec::required("project_dir"),
        FieldSpec::required("profiles_dir"),
        FieldSpec::required("target"),
        FieldSpec::required("target_path"),
    ],
};
const DBT_NODE_MESSAGE: RecordSpec = RecordSpec {
    name: "DbtNodeMessage",
    doc: "One dbt log message attached to a dbt node.",
    fields: &[FieldSpec::required("level"), FieldSpec::required("message")],
};
const DBT_COMMAND_EXECUTION_RESULT: RecordSpec = RecordSpec {
    name: "DbtCommandExecutionResult",
    doc: "dbt command execution output from streamed JSON events.",
    fields: &[
        FieldSpec::required("returncode"),
        FieldSpec::new("node_results", FieldDefault::EmptyTuple),
    ],
};
const DBT_LS_NODE: RecordSpec = RecordSpec {
    name: "DbtLsNode",
    doc: "One node returned by `dbt ls --output json`.",
    fields: &[
        FieldSpec::required("unique_id"),
        FieldSpec::new("resource_type", FieldDefault::None),
        FieldSpec::new("package_name", FieldDefault::None),
        FieldSpec::new("name", FieldDefault::None),
        FieldSpec::new("fqn", FieldDefault::EmptyTuple),
        FieldSpec::new("original_file_path", FieldDefault::None),
    ],
};
const DBT_INTEROP_PARSED_ARGS: RecordSpec = RecordSpec {
    name: "DbtInteropParsedArgs",
    doc: "Declared `sqb dbt` flags parsed by argparse, before routing to buckets.",
    fields: &[
        FieldSpec::new("select", FieldDefault::EmptyTuple),
        FieldSpec::new("exclude", FieldDefault::EmptyTuple),
        FieldSpec::new("vars", FieldDefault::None),
        FieldSpec::new("threads", FieldDefault::None),
        FieldSpec::new("full_refresh", FieldDefault::False),
        FieldSpec::new("event_time_start", FieldDefault::None),
        FieldSpec::new("event_time_end", FieldDefault::None),
        FieldSpec::new("project_dir", FieldDefault::None),
        FieldSpec::new("profiles_dir", FieldDefault::None),
        FieldSpec::new("profile", FieldDefault::None),
        FieldSpec::new("target", FieldDefault::None),
        FieldSpec::new("target_path", FieldDefault::None),
        FieldSpec::new("state", FieldDefault::None),
        FieldSpec::new("indirect_selection", FieldDefault::None),
        FieldSpec::new("defer", FieldDefault::False),
        FieldSpec::new("start_cursor_ts", FieldDefault::None),
        FieldSpec::new("end_cursor_ts", FieldDefault::None),
        FieldSpec::new("start_cursor_int", FieldDefault::None),
        FieldSpec::new("end_cursor_int", FieldDefault::None),
        FieldSpec::new("defer_to", FieldDefault::None),
        FieldSpec::new("fail_fast", FieldDefault::False),
        FieldSpec::new("hard_copy", FieldDefault::False),
        FieldSpec::new("dbt_passthrough", FieldDefault::EmptyTuple),
    ],
};
const DBT_INTEROP_ROUTED_ARGS: RecordSpec = RecordSpec {
    name: "DbtInteropRoutedArgs",
    doc: "Arguments split for a future `sqb dbt` command execution.",
    fields: &[
        FieldSpec::required("command"),
        FieldSpec::new("select", FieldDefault::EmptyTuple),
        FieldSpec::new("exclude", FieldDefault::EmptyTuple),
        FieldSpec::new("dbt_args", FieldDefault::EmptyTuple),
        FieldSpec::new("sqlbuild_args", FieldDefault::EmptyTuple),
    ],
};
const DBT_INTEROP_SELECTION_RESULT: RecordSpec = RecordSpec {
    name: "DbtInteropSelectionResult",
    doc: "SQLBuild-side selection produced from `sqb dbt` selectors.",
    fields: &[
        FieldSpec::new("sqlbuild_model_names", FieldDefault::EmptyTuple),
        FieldSpec::new("dbt_required_unique_ids", FieldDefault::EmptyTuple),
        FieldSpec::new("dbt_anchor_terms", FieldDefault::EmptyTuple),
        FieldSpec::new("dbt_anchor_unique_ids_by_term", FieldDefault::EmptyDict),
        FieldSpec::new("path_translations", FieldDefault::EmptyTuple),
    ],
};
const DBT_INTEROP_PLAN: RecordSpec = RecordSpec {
    name: "DbtInteropPlan",
    doc: "Plan output for one future `sqb dbt` command.",
    fields: &[
        FieldSpec::required("command"),
        FieldSpec::required("dbt_command_argv"),
        FieldSpec::required("dbt_selected_nodes"),
        FieldSpec::required("dbt_selected_unique_ids"),
        FieldSpec::required("sqlbuild_command_argvs"),
        FieldSpec::required("selection"),
        FieldSpec::new("sqlbuild_plan_output", FieldDefault::None),
        FieldSpec::new("dbt_required_selector_terms", FieldDefault::EmptyTuple),
        FieldSpec::new("supplemental_dbt_command_argvs", FieldDefault::EmptyTuple),
        FieldSpec::new("dbt_skip_reason", FieldDefault::None),
        FieldSpec::new("sqlbuild_skip_reason", FieldDefault::None),
        FieldSpec::new("warnings", FieldDefault::EmptyTuple),
    ],
};
const DBT_INTEROP_EXECUTION_REQUEST: RecordSpec = RecordSpec {
    name: "DbtInteropExecutionRequest",
    doc: "Caller inputs for one dbt interop execution command.",
    fields: &[
        FieldSpec::required("command"),
        FieldSpec::required("project_dir"),
        FieldSpec::required("args"),
        FieldSpec::new("dbt_runner", FieldDefault::None),
        FieldSpec::new("dbt_executable", FieldDefault::None),
        FieldSpec::new("sqlbuild_executable", FieldDefault::Text("sqb")),
        FieldSpec::new("no_sql_validation", FieldDefault::False),
        FieldSpec::new("fail_fast", FieldDefault::False),
        FieldSpec::new("on_progress", FieldDefault::None),
        FieldSpec::new("progress_stream", FieldDefault::None),
        FieldSpec::new("dbt_stdout_stream", FieldDefault::None),
        FieldSpec::new("use_color", FieldDefault::False),
        FieldSpec::new("verbose", FieldDefault::False),
        FieldSpec::new("json_output", FieldDefault::False),
    ],
};
const DBT_INTEROP_INVOCATION: RecordSpec = RecordSpec {
    name: "DbtInteropInvocation",
    doc: "Resolved execution environment for one dbt interop command.",
    fields: &[
        FieldSpec::required("dbt_executable"),
        FieldSpec::required("output_stream"),
        FieldSpec::required("dbt_output_stream"),
        FieldSpec::required("routed"),
        FieldSpec::required("discovered_inputs"),
        FieldSpec::required("effective_sqlbuild_args"),
        FieldSpec::required("dbt_options"),
        FieldSpec::required("dbt_vars"),
        FieldSpec::required("runner"),
    ],
};
const DBT_INTEROP_COMPILED_PROJECT: RecordSpec = RecordSpec {
    name: "DbtInteropCompiledProject",
    doc: "Adapter and compiled SQLBuild project for dbt interop planning.",
    fields: &[
        FieldSpec::required("adapter_name"),
        FieldSpec::required("adapter"),
        FieldSpec::required("project"),
    ],
};
const DBT_INTEROP_PLAN_RESOLUTION: RecordSpec = RecordSpec {
    name: "DbtInteropPlanResolution",
    doc: "Combined dbt graph and the resolved interop plan.",
    fields: &[FieldSpec::required("graph"), FieldSpec::required("plan")],
};
const DBT_INTEROP_COMMAND_ARGS: RecordSpec = RecordSpec {
    name: "DbtInteropCommandArgs",
    doc: "Selection, passthrough args, and executables for a dbt interop command.",
    fields: &[
        FieldSpec::required("select"),
        FieldSpec::new("exclude", FieldDefault::EmptyTuple),
        FieldSpec::new("dbt_command_args", FieldDefault::EmptyTuple),
        FieldSpec::new("sqlbuild_command_args", FieldDefault::EmptyTuple),
        FieldSpec::new("dbt_executable", FieldDefault::Text("dbt")),
        FieldSpec::new("sqlbuild_executable", FieldDefault::Text("sqb")),
    ],
};
const DBT_PLAN_ENVIRONMENT: RecordSpec = RecordSpec {
    name: "DbtPlanEnvironment",
    doc: "Compiled project and adapter environment for dbt plan-output builds.",
    fields: &[
        FieldSpec::required("project_dir"),
        FieldSpec::required("discovered_inputs"),
        FieldSpec::required("project"),
        FieldSpec::required("adapter"),
        FieldSpec::required("adapter_name"),
    ],
};
const DBT_SQLBUILD_PLAN_REQUEST: RecordSpec = RecordSpec {
    name: "DbtSqlbuildPlanRequest",
    doc: "Selection and native planner inputs for a SQLBuild plan-output build.",
    fields: &[
        FieldSpec::required("selected_model_names"),
        FieldSpec::required("sqlbuild_args"),
        FieldSpec::new("external_blocked_model_names", FieldDefault::EmptyTuple),
    ],
};

/// dbt manifest indexes, interop arguments, plans and invocations.
const DBT_RECORDS: &[RecordSpec] = &[
    DBT_MANIFEST_MODEL,
    DBT_MANIFEST_SOURCE,
    DBT_MANIFEST_SEED,
    DBT_MANIFEST_INDEX,
    DBT_CLI_CONFIG_OVERRIDES,
    RESOLVED_DBT_CONFIG,
    DBT_NODE_MESSAGE,
    DBT_COMMAND_EXECUTION_RESULT,
    DBT_LS_NODE,
    DBT_INTEROP_PARSED_ARGS,
    DBT_INTEROP_ROUTED_ARGS,
    DBT_INTEROP_SELECTION_RESULT,
    DBT_INTEROP_PLAN,
    DBT_INTEROP_EXECUTION_REQUEST,
    DBT_INTEROP_INVOCATION,
    DBT_INTEROP_COMPILED_PROJECT,
    DBT_INTEROP_PLAN_RESOLUTION,
    DBT_INTEROP_COMMAND_ARGS,
    DBT_PLAN_ENVIRONMENT,
    DBT_SQLBUILD_PLAN_REQUEST,
];

const SOURCE_FRESHNESS_RENDERERS: RecordSpec = RecordSpec {
    name: "SourceFreshnessRenderers",
    doc: "Adapter SQL renderers used when writing source freshness rows.",
    fields: &[
        FieldSpec::required("render_qualified_name"),
        FieldSpec::required("render_framework_type"),
        FieldSpec::required("render_insert_records_sql"),
        FieldSpec::new("render_create_table_sql", FieldDefault::None),
        FieldSpec::new("render_create_index_sqls", FieldDefault::None),
    ],
};
const SOURCE_FRESHNESS_OBSERVATION: RecordSpec = RecordSpec {
    name: "SourceFreshnessObservation",
    doc: "One comparable observation whose datetime values are always aware UTC.",
    fields: &[
        FieldSpec::required("source_name"),
        FieldSpec::required("strategy"),
        FieldSpec::required("data_version"),
        FieldSpec::required("value_kind"),
        FieldSpec::required("observed_at"),
    ],
};
const SOURCE_FRESHNESS_UNKNOWN: RecordSpec = RecordSpec {
    name: "SourceFreshnessUnknown",
    doc: "One source whose freshness could not be observed, with the reason.",
    fields: &[
        FieldSpec::required("source_name"),
        FieldSpec::required("reason"),
        FieldSpec::required("message"),
    ],
};
const ADAPTER_SOURCE_FRESHNESS_BATCH: RecordSpec = RecordSpec {
    name: "AdapterSourceFreshnessBatch",
    doc: "Per-source outcomes of one batched adapter metadata observation.",
    fields: &[
        FieldSpec::new("observations", FieldDefault::EmptyDict),
        FieldSpec::new("unknown", FieldDefault::EmptyDict),
    ],
};
const SOURCE_FRESHNESS_IDENTITY: RecordSpec = RecordSpec {
    name: "SourceFreshnessIdentity",
    doc: "Stable identity for one source freshness stream in direct state.",
    fields: &[
        FieldSpec::required("source_name"),
        FieldSpec::required("target_database"),
        FieldSpec::required("target_schema"),
        FieldSpec::required("target_name"),
    ],
};
const SOURCE_FRESHNESS_SET: RecordSpec = RecordSpec {
    name: "SourceFreshnessSet",
    doc: "Latest direct source freshness records for one target schema.",
    fields: &[
        FieldSpec::required("schema"),
        FieldSpec::required("records"),
    ],
};
const DIRECT_SOURCE_FRESHNESS_PROPAGATION_RESULT: RecordSpec = RecordSpec {
    name: "DirectSourceFreshnessPropagationResult",
    doc: "Downstream model impact derived from direct source freshness roots.",
    fields: &[
        FieldSpec::new("changed_source_model_names", FieldDefault::EmptyDict),
        FieldSpec::new("unknown_source_model_names", FieldDefault::EmptyDict),
        FieldSpec::new("error_source_model_names", FieldDefault::EmptyDict),
        FieldSpec::new("stale_model_names", FieldDefault::EmptyFrozenset),
        FieldSpec::new("blocked_model_names", FieldDefault::EmptyFrozenset),
    ],
};

/// Source freshness observations, identities and planning results.
const FRESHNESS_RECORDS: &[RecordSpec] = &[
    SOURCE_FRESHNESS_RENDERERS,
    SOURCE_FRESHNESS_OBSERVATION,
    SOURCE_FRESHNESS_UNKNOWN,
    ADAPTER_SOURCE_FRESHNESS_BATCH,
    SOURCE_FRESHNESS_IDENTITY,
    SOURCE_FRESHNESS_SET,
    DIRECT_SOURCE_FRESHNESS_PROPAGATION_RESULT,
];

const SOURCE_FRESHNESS_AGE_STATUS: EnumSpec = EnumSpec {
    name: "SourceFreshnessAgeStatus",
    doc: "",
    members: &[
        ("PASS", "pass"),
        ("WARN", "warn"),
        ("ERROR", "error"),
        ("UNKNOWN", "unknown"),
    ],
};
const SOURCE_FRESHNESS_UNKNOWN_REASON: EnumSpec = EnumSpec {
    name: "SourceFreshnessUnknownReason",
    doc: "Why one source's freshness could not be observed.",
    members: &[
        ("UNAVAILABLE", "unavailable"),
        ("ERROR", "error"),
        ("NO_FRESHNESS_CONFIG", "no_freshness_config"),
    ],
};

/// Source freshness age statuses and unknown reasons.
const FRESHNESS_ENUMS: &[EnumSpec] =
    &[SOURCE_FRESHNESS_AGE_STATUS, SOURCE_FRESHNESS_UNKNOWN_REASON];

const QUERY_COST_OBSERVATION: RecordSpec = RecordSpec {
    name: "QueryCostObservation",
    doc: "",
    fields: &[
        FieldSpec::required("query_id"),
        FieldSpec::required("warehouse_name"),
        FieldSpec::required("warehouse_size"),
        FieldSpec::required("warehouse_type"),
        FieldSpec::required("cluster_number"),
        FieldSpec::required("started_at"),
        FieldSpec::required("completed_at"),
        FieldSpec::required("execution_ms"),
        FieldSpec::required("bytes_scanned"),
        FieldSpec::required("execution_status"),
        FieldSpec::new("run_id", FieldDefault::None),
        FieldSpec::new("resource_type", FieldDefault::None),
        FieldSpec::new("resource_name", FieldDefault::None),
    ],
};
const RESOURCE_COST: RecordSpec = RecordSpec {
    name: "ResourceCost",
    doc: "",
    fields: &[
        FieldSpec::required("resource_type"),
        FieldSpec::required("resource_name"),
        FieldSpec::required("warehouse_name"),
        FieldSpec::required("warehouse_size"),
        FieldSpec::required("warehouse_type"),
        FieldSpec::required("cluster_number"),
        FieldSpec::required("query_count"),
        FieldSpec::required("attributed_seconds"),
        FieldSpec::required("bytes_scanned"),
        FieldSpec::required("estimated_compute_credits"),
        FieldSpec::required("estimated_usd"),
    ],
};
const COST_RUN_RECORD: RecordSpec = RecordSpec {
    name: "CostRunRecord",
    doc: "",
    fields: &[
        FieldSpec::required("run_id"),
        FieldSpec::required("adapter_name"),
        FieldSpec::required("target_name"),
        FieldSpec::required("build_status"),
        FieldSpec::required("started_at"),
        FieldSpec::required("completed_at"),
        FieldSpec::required("cost"),
        FieldSpec::new("version", FieldDefault::Int(1)),
        FieldSpec::new("had_executable_work", FieldDefault::None),
    ],
};
const STATEMENT_LEDGER_ENTRY: RecordSpec = RecordSpec {
    name: "StatementLedgerEntry",
    doc: "",
    fields: &[
        FieldSpec::required("statement_id"),
        FieldSpec::required("run_id"),
        FieldSpec::required("resource_type"),
        FieldSpec::required("resource_name"),
        FieldSpec::required("phase"),
        FieldSpec::required("attempt"),
        FieldSpec::required("query_id"),
        FieldSpec::required("status"),
        FieldSpec::required("started_at"),
        FieldSpec::required("completed_at"),
    ],
};

/// Run-cost observations, allocations, run records and statement ledger entries.
const COST_RECORDS: &[RecordSpec] = &[
    QUERY_COST_OBSERVATION,
    RESOURCE_COST,
    COST_RUN_RECORD,
    STATEMENT_LEDGER_ENTRY,
];

const COMPILE_LINEAGE_MODE: EnumSpec = EnumSpec {
    name: "CompileLineageMode",
    doc: "Column lineage mode for compile output.",
    members: &[("FAST", "fast"), ("RICH", "rich"), ("NONE", "none")],
};
const CLI_COMMAND: EnumSpec = EnumSpec {
    name: "CliCommand",
    doc: "",
    members: &[
        ("COMPILE", "compile"),
        ("DAG", "dag"),
        ("PLAN", "plan"),
        ("FRESHNESS", "freshness"),
        ("BUILD", "build"),
        ("TEST", "test"),
        ("CHECK", "check"),
        ("AUDIT", "audit"),
        ("LOAD", "load"),
        ("SEED", "seed"),
        ("CLONE", "clone"),
        ("DIFF", "diff"),
        ("REVIEW", "review"),
        ("DEBUG", "debug"),
        ("LOCATION", "location"),
        ("LINEAGE", "lineage"),
        ("QUERY", "query"),
        ("COST", "cost"),
        ("CLEAN", "clean"),
        ("JANITOR", "janitor"),
        ("INIT", "init"),
        ("PLAYGROUND", "playground"),
        ("SCENARIO", "scenario"),
        ("DBT", "dbt"),
        ("SKILLS", "skills"),
        ("FORMAT", "format"),
        ("RULES", "rules"),
        ("SCOPE", "scope"),
        ("CONTRACT", "contract"),
        ("RENAME", "rename"),
        ("MV", "mv"),
        ("RECALL", "recall"),
    ],
};
const PLAYGROUND_TEMPLATE: EnumSpec = EnumSpec {
    name: "PlaygroundTemplate",
    doc: "Available `sqb playground` templates.",
    members: &[
        ("WAFFLE_SHOP", "waffle_shop"),
        ("LOADER_WAFFLE_SHOP", "loader_waffle_shop"),
        ("DAGSTER", "dagster"),
        ("RIVERS", "rivers"),
        ("PYTHON_NODES", "python_nodes"),
    ],
};

const AUTHORED_SQL_SET: RecordSpec = RecordSpec {
    name: "AuthoredSqlSet",
    doc: "Set literal entries retained in authored order until validation.",
    fields: &[FieldSpec::required("values")],
};
const AUTHORED_SQL_VALUE_CALL: RecordSpec = RecordSpec {
    name: "AuthoredSqlValueCall",
    doc: "Source-preserving representation of an authored ``constant(...)`` call.",
    fields: &[FieldSpec::required("arguments")],
};
const SQL_VALUE_LIMITS: RecordSpec = RecordSpec {
    name: "SqlValueLimits",
    doc: "Safety limits applied while normalizing recursively authored values.",
    fields: &[
        FieldSpec::new("max_depth", FieldDefault::Int(32)),
        FieldSpec::new("max_elements", FieldDefault::Int(10000)),
        FieldSpec::new("max_size", FieldDefault::Int(1000000)),
    ],
};

/// Authored typed SQL values and normalization limits.
const SQL_VALUE_RECORDS: &[RecordSpec] =
    &[AUTHORED_SQL_SET, AUTHORED_SQL_VALUE_CALL, SQL_VALUE_LIMITS];

const PARTIAL_BUILD_PHASE_TIMINGS: RecordSpec = RecordSpec {
    name: "PartialBuildPhaseTimings",
    doc: "Available disjoint phase durations after an exceptional build.",
    fields: &[
        FieldSpec::new("compile_seconds", FieldDefault::None),
        FieldSpec::new("planning_seconds", FieldDefault::None),
        FieldSpec::new("connection_preparation_seconds", FieldDefault::None),
        FieldSpec::new("schema_preparation_seconds", FieldDefault::None),
        FieldSpec::new("execution_seconds", FieldDefault::None),
        FieldSpec::new("cost_collection_seconds", FieldDefault::None),
        FieldSpec::new("total_seconds", FieldDefault::None),
    ],
};

/// Phase durations available after an exceptional build.
const DIAGNOSTIC_RECORDS: &[RecordSpec] = &[PARTIAL_BUILD_PHASE_TIMINGS];

/// `sqb` commands, compile lineage modes and playground templates.
const CLI_ENUMS: &[EnumSpec] = &[COMPILE_LINEAGE_MODE, CLI_COMMAND, PLAYGROUND_TEMPLATE];
const DBT_INTEROP_COMMAND: EnumSpec = EnumSpec {
    name: "DbtInteropCommand",
    doc: "dbt interop commands with SQLBuild participation.",
    members: &[
        ("PLAN", "plan"),
        ("RUN", "run"),
        ("BUILD", "build"),
        ("DEBUG", "debug"),
    ],
};
const DBT_INTEROP_SKIP_REASON: EnumSpec = EnumSpec {
    name: "DbtInteropSkipReason",
    doc: "Reason one side of a dbt interop plan has no work.",
    members: &[
        ("NO_DBT_WORK", "no_dbt_work"),
        ("NO_SQLBUILD_WORK", "no_sqlbuild_work"),
    ],
};

/// dbt interop commands and skip reasons.
const DBT_ENUMS: &[EnumSpec] = &[DBT_INTEROP_COMMAND, DBT_INTEROP_SKIP_REASON];

const COMPLETION_STATE: EnumSpec = EnumSpec {
    name: "CompletionState",
    doc: "Overall outcome state for a completion summary line.",
    members: &[("OK", "ok"), ("WARN", "warn"), ("FAIL", "fail")],
};

/// Completion summary outcomes.
const PRESENTATION_ENUMS: &[EnumSpec] = &[COMPLETION_STATE];
