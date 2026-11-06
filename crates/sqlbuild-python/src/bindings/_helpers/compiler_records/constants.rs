//! Layouts of the compiler, lint and config value records hosted natively.

use sqlbuild_host::entries::models::UserCodeHost;

use crate::bindings::_helpers::records::ids::RecordId;
use crate::bindings::_helpers::records::models::{FieldDefault, FieldSpec, RecordSpec};

const R_NATIVE_PIVOT_TABLES: RecordSpec = RecordSpec {
    name: "NativePivotTables",
    doc: "The relation facts Python's dynamic pivot proofs read, for proofs outside a session.",
    fields: &[
        FieldSpec::required("dialect"),
        FieldSpec::required("column_types_by_table"),
        FieldSpec::required("authoritative_column_types_by_table"),
        FieldSpec::required("column_nullability_by_table"),
        FieldSpec::required("dynamic_families_by_table"),
    ],
};
const R_NATIVE_COLUMN_QUERY: RecordSpec = RecordSpec {
    name: "NativeColumnQuery",
    doc: "One query outside the session whose output columns Python's analysis inferred.",
    fields: &[
        FieldSpec::required("sql"),
        FieldSpec::required("mode"),
        FieldSpec::new("placeholders", FieldDefault::None),
        FieldSpec::new("references", FieldDefault::EmptyTuple),
        FieldSpec::new("recover_cte_facts", FieldDefault::True),
    ],
};
const R_NATIVE_QUERY_COLUMNS: RecordSpec = RecordSpec {
    name: "NativeQueryColumns",
    doc: "One query's analysis: whether it succeeded, its columns and whether it projects a star.",
    fields: &[
        FieldSpec::required("succeeded"),
        FieldSpec::required("columns"),
        FieldSpec::required("has_star"),
    ],
};
const R_CONTRACT_FINDING: RecordSpec = RecordSpec {
    name: "ContractFinding",
    doc: "",
    fields: &[
        FieldSpec::required("resource_type"),
        FieldSpec::required("resource_name"),
        FieldSpec::required("kind"),
        FieldSpec::required("message"),
        FieldSpec::new("column_name", FieldDefault::None),
        FieldSpec::new("declared_type", FieldDefault::None),
        FieldSpec::new("physical_type", FieldDefault::None),
    ],
};
const R_CONTRACT_EVIDENCE: RecordSpec = RecordSpec {
    name: "ContractEvidence",
    doc: "",
    fields: &[
        FieldSpec::required("resource_type"),
        FieldSpec::required("resource_name"),
        FieldSpec::required("database"),
        FieldSpec::required("schema"),
        FieldSpec::required("relation"),
        FieldSpec::required("declared_columns"),
        FieldSpec::required("physical_columns"),
        FieldSpec::required("source_path"),
        FieldSpec::new("dynamic_columns", FieldDefault::EmptyTuple),
        FieldSpec::new("findings", FieldDefault::EmptyTuple),
    ],
};
const R_CONTRACT_ADOPTION_RESULT: RecordSpec = RecordSpec {
    name: "ContractAdoptionResult",
    doc: "",
    fields: &[
        FieldSpec::required("from_target"),
        FieldSpec::required("evidence"),
        FieldSpec::new("written_paths", FieldDefault::EmptyTuple),
    ],
};
const R_CONTRACT_VALIDATION_RESULT: RecordSpec = RecordSpec {
    name: "ContractValidationResult",
    doc: "Diagnostics emitted by model contract validation.",
    fields: &[FieldSpec::new("diagnostics", FieldDefault::EmptyTuple)],
};
const R_LINEAGE_GRAPH_VIEWS: RecordSpec = RecordSpec {
    name: "LineageGraphViews",
    doc: "A native project graph's lineage edges and selector indexes as Python dicts.",
    fields: &[
        FieldSpec::required("upstream_deps"),
        FieldSpec::required("downstream_deps"),
        FieldSpec::required("tag_index"),
        FieldSpec::required("path_index"),
        FieldSpec::required("all_keys"),
    ],
};
const R_ATTACHED_AUDIT_GATE_EDGE: RecordSpec = RecordSpec {
    name: "AttachedAuditGateEdge",
    doc: "One ordering edge: the gated node waits for a resource its attached audit reads.",
    fields: &[
        FieldSpec::required("audit_name"),
        FieldSpec::required("target"),
        FieldSpec::required("gated"),
        FieldSpec::required("read"),
    ],
};
const R_HOOK_READ_EDGE: RecordSpec = RecordSpec {
    name: "HookReadEdge",
    doc: "One ordering edge: a model waits for a resource one of its hooks reads.",
    fields: &[
        FieldSpec::required("hook_name"),
        FieldSpec::required("gated"),
        FieldSpec::required("read"),
        FieldSpec::new(
            "hook_type",
            FieldDefault::NativeMember(RecordId::HookReadType, "PYTHON"),
        ),
        FieldSpec::new("hook_path", FieldDefault::None),
    ],
};
const R_QUALIFIED_LINEAGE_COLUMN: RecordSpec = RecordSpec {
    name: "QualifiedLineageColumn",
    doc: "A resource-qualified column in the collapsed SQLBuild lineage graph.",
    fields: &[
        FieldSpec::required("resource_type"),
        FieldSpec::required("resource_name"),
        FieldSpec::required("column_name"),
    ],
};
const R_COLUMN_LINEAGE_SOURCE: RecordSpec = RecordSpec {
    name: "ColumnLineageSource",
    doc: "One upstream column dependency for an output column.",
    fields: &[
        FieldSpec::required("resource_type"),
        FieldSpec::required("resource_name"),
        FieldSpec::required("column_name"),
    ],
};
const R_COLUMN_LINEAGE_EDGE: RecordSpec = RecordSpec {
    name: "ColumnLineageEdge",
    doc: "One collapsed SQLBuild lineage graph edge.",
    fields: &[
        FieldSpec::required("source"),
        FieldSpec::required("target"),
        FieldSpec::new(
            "transform_kind",
            FieldDefault::NativeMember(RecordId::ColumnTransformKind, "UNKNOWN"),
        ),
        FieldSpec::new(
            "confidence",
            FieldDefault::NativeMember(RecordId::ColumnLineageConfidence, "UNKNOWN"),
        ),
    ],
};
const R_DIRECT_SEMANTIC_COLUMN_USE: RecordSpec = RecordSpec {
    name: "DirectSemanticColumnUse",
    doc: "One direct non-projection use of a resolved resource column.",
    fields: &[
        FieldSpec::required("consumer_model"),
        FieldSpec::required("context"),
        FieldSpec::required("expression_sql"),
        FieldSpec::required("source"),
        FieldSpec::new(
            "confidence",
            FieldDefault::NativeMember(RecordId::ColumnLineageConfidence, "UNKNOWN"),
        ),
        FieldSpec::new("line", FieldDefault::None),
        FieldSpec::new("column", FieldDefault::None),
    ],
};
const R_COLUMN_LINEAGE: RecordSpec = RecordSpec {
    name: "ColumnLineage",
    doc: "Lineage for one output column in one model.",
    fields: &[
        FieldSpec::required("output_column"),
        FieldSpec::required("transform_kind"),
        FieldSpec::required("expression_sql"),
        FieldSpec::required("upstream_columns"),
        FieldSpec::new(
            "nullability",
            FieldDefault::NativeMember(RecordId::InferredNullability, "UNKNOWN"),
        ),
        FieldSpec::new(
            "confidence",
            FieldDefault::NativeMember(RecordId::ColumnLineageConfidence, "UNKNOWN"),
        ),
    ],
};
const R_MODEL_COLUMN_LINEAGE: RecordSpec = RecordSpec {
    name: "ModelColumnLineage",
    doc: "Column lineage extracted for one compiled model.",
    fields: &[
        FieldSpec::required("model_name"),
        FieldSpec::required("columns"),
        FieldSpec::new("has_star", FieldDefault::False),
    ],
};
const R_REFACTOR_PROJECT: RecordSpec = RecordSpec {
    name: "RefactorProject",
    doc: "Compiler facts of one project, as the refactoring planner reads them.",
    fields: &[
        FieldSpec::required("project_dir"),
        FieldSpec::required("graph"),
        FieldSpec::required("discovered"),
    ],
};
const R_MANUAL_LOCATION: RecordSpec = RecordSpec {
    name: "ManualLocation",
    doc: "A location a refactoring cannot rewrite safely.",
    fields: &[
        FieldSpec::required("path"),
        FieldSpec::required("line"),
        FieldSpec::required("column"),
        FieldSpec::required("reason"),
    ],
};
const R_NATIVE_SQL_TEST_FAILURE: RecordSpec = RecordSpec {
    name: "NativeSqlTestFailure",
    doc: "The error one test's assembly raises: its kind and message.",
    fields: &[FieldSpec::required("kind"), FieldSpec::required("message")],
};
const R_NATIVE_SQL_TEST_ASSEMBLY: RecordSpec = RecordSpec {
    name: "NativeSqlTestAssembly",
    doc: "One natively assembled test, its keyed diagnostics, and the error its assembly raises.",
    fields: &[
        FieldSpec::required("test"),
        FieldSpec::required("diagnostics"),
        FieldSpec::new("failure", FieldDefault::None),
    ],
};
const R_HEADER_SPAN: RecordSpec = RecordSpec {
    name: "HeaderSpan",
    doc: "A located DSL header region inside one file.",
    fields: &[
        FieldSpec::required("kind"),
        FieldSpec::required("start"),
        FieldSpec::required("end"),
    ],
};
const R_LINT_FILE_ROLE: RecordSpec = RecordSpec {
    name: "LintFileRole",
    doc: "Path-derived classification of one lint input, derived once per file.",
    fields: &[
        FieldSpec::new("declaration_kind", FieldDefault::None),
        FieldSpec::new("in_project", FieldDefault::False),
        FieldSpec::new("in_hook_directory", FieldDefault::False),
    ],
};
const R_INTERPOLATION_SITE: RecordSpec = RecordSpec {
    name: "InterpolationSite",
    doc: "One sqlbuild interpolation occurrence replaced by a unique sentinel.",
    fields: &[
        FieldSpec::required("sentinel"),
        FieldSpec::required("neutralized_start"),
        FieldSpec::required("neutralized_end"),
        FieldSpec::required("original_start"),
        FieldSpec::required("original_end"),
        FieldSpec::required("original_text"),
    ],
};
const R_LINT_BODY: RecordSpec = RecordSpec {
    name: "LintBody",
    doc: "One authored SQL body prepared for linting, with its expansion spans.",
    fields: &[
        FieldSpec::required("file_path"),
        FieldSpec::required("body_start"),
        FieldSpec::required("body_end"),
        FieldSpec::required("lint_text"),
        FieldSpec::required("passes"),
        FieldSpec::new("external_identifiers", FieldDefault::EmptyTuple),
        FieldSpec::new("dependency_identifiers", FieldDefault::EmptyTuple),
        FieldSpec::new("externally_referenced_ctes", FieldDefault::EmptyTuple),
        FieldSpec::new("allows_ceremonial_select", FieldDefault::False),
        FieldSpec::new("allows_dynamic_output_star", FieldDefault::False),
        FieldSpec::new("allows_empty_fixture_star", FieldDefault::False),
        FieldSpec::new("dependency_relations", FieldDefault::EmptyTuple),
    ],
};
const R_LINT_EDIT: RecordSpec = RecordSpec {
    name: "LintEdit",
    doc: "One deterministic authored-source edit proposed by a lint rule.",
    fields: &[
        FieldSpec::required("file_path"),
        FieldSpec::required("code"),
        FieldSpec::required("start"),
        FieldSpec::required("end"),
        FieldSpec::required("replacement"),
    ],
};
const R_LINT_VIOLATION: RecordSpec = RecordSpec {
    name: "LintViolation",
    doc: "One lint diagnostic reported against an authored file.",
    fields: &[
        FieldSpec::required("file_path"),
        FieldSpec::required("line"),
        FieldSpec::required("column"),
        FieldSpec::required("code"),
        FieldSpec::required("message"),
        FieldSpec::required("severity"),
        FieldSpec::required("engine"),
        FieldSpec::new("end_line", FieldDefault::None),
        FieldSpec::new("end_column", FieldDefault::None),
        FieldSpec::new("remediation", FieldDefault::None),
        FieldSpec::new("fix", FieldDefault::None),
        FieldSpec::new("fix_unavailable_reason", FieldDefault::None),
    ],
};
const R_RULE_FIX_RESULT: RecordSpec = RecordSpec {
    name: "RuleFixResult",
    doc: "One applied, refused, or unavailable semantic Rule fix.",
    fields: &[
        FieldSpec::required("file_path"),
        FieldSpec::required("code"),
        FieldSpec::required("line"),
        FieldSpec::required("status"),
        FieldSpec::required("reason"),
    ],
};
const R_LINT_CONFIG: RecordSpec = RecordSpec {
    name: "LintConfig",
    doc: "Resolved lint and format configuration for one run.",
    fields: &[
        FieldSpec::new("native_enabled", FieldDefault::True),
        FieldSpec::new("max_description_lines", FieldDefault::Int(10)),
        FieldSpec::new("line_width", FieldDefault::Int(100)),
        FieldSpec::new("dialect", FieldDefault::Text("generic")),
        FieldSpec::new("enabled_native_rules", FieldDefault::None),
        FieldSpec::new("ignored_native_rules", FieldDefault::EmptyTuple),
        FieldSpec::new("header_rules_enabled", FieldDefault::True),
        FieldSpec::new("max_literal_length", FieldDefault::Int(100)),
        FieldSpec::new("max_ranking_order_by", FieldDefault::Int(5)),
        FieldSpec {
            name: "relation_keys",
            default: FieldDefault::EmptyDict,
            compared: false,
            shown: false,
        },
    ],
};
const R_LINT_RUN_RESULT: RecordSpec = RecordSpec {
    name: "LintRunResult",
    doc: "Aggregated outcome of one lint or format run.",
    fields: &[
        FieldSpec::required("files_checked"),
        FieldSpec::required("violations"),
        FieldSpec::required("formatted_files"),
        FieldSpec {
            name: "source_texts",
            default: FieldDefault::EmptyDict,
            compared: false,
            shown: false,
        },
        FieldSpec::new("unexpandable", FieldDefault::EmptyDict),
    ],
};
const R_COMPILE_FACTS: RecordSpec = RecordSpec {
    name: "CompileFacts",
    doc: "Per-model outputs and per-owner diagnostics of one compilation.",
    fields: &[
        FieldSpec::required("models"),
        FieldSpec::required("model_paths"),
        FieldSpec::required("diagnostics"),
    ],
};
const R_FIX_VERDICT: RecordSpec = RecordSpec {
    name: "FixVerdict",
    doc: "Edited files that fail verification, and whether any change is unattributable.",
    fields: &[
        FieldSpec::required("failing"),
        FieldSpec::required("unattributed"),
    ],
};
const R_DISPLAY_OPTIONS: RecordSpec = RecordSpec {
    name: "DisplayOptions",
    doc: "Options controlling bounded human output.",
    fields: &[
        FieldSpec::new("max_entries_per_section", FieldDefault::Int(20)),
        FieldSpec::new("overflow_flag", FieldDefault::Text("--verbose")),
    ],
};
const R_COMPILE_PIPELINE_OPTIONS: RecordSpec = RecordSpec {
    name: "CompilePipelineOptions",
    doc: "Selection, deferral, and planning options for one compile pipeline run.",
    fields: &[
        FieldSpec::new("selected_target", FieldDefault::None),
        FieldSpec::new("no_sql_validation", FieldDefault::False),
        FieldSpec::new("defer_to", FieldDefault::None),
        FieldSpec::new("defer_sources_to", FieldDefault::None),
        FieldSpec::new("source_deferral_enabled", FieldDefault::True),
        FieldSpec::new("select", FieldDefault::EmptyTuple),
        FieldSpec::new("exclude", FieldDefault::EmptyTuple),
        FieldSpec::new("cursor_overrides", FieldDefault::None),
        FieldSpec::new("full_refresh", FieldDefault::False),
        FieldSpec::new("auto_load_sources", FieldDefault::False),
        FieldSpec::new("reload_sources", FieldDefault::False),
        FieldSpec::new("connection_config", FieldDefault::None),
        FieldSpec::new("cli_vars", FieldDefault::None),
        FieldSpec::new("external_sql_reference_resolver", FieldDefault::None),
        FieldSpec::new("resolve_python_run_selectors", FieldDefault::False),
        FieldSpec::new("no_cache", FieldDefault::False),
        FieldSpec::new("max_microbatches", FieldDefault::None),
        FieldSpec::new("concurrency", FieldDefault::None),
        FieldSpec::new("selection_diagnostics", FieldDefault::False),
        FieldSpec::new("plan_sql_tests", FieldDefault::True),
        FieldSpec::new("record_migration_fingerprints", FieldDefault::True),
        FieldSpec::new("accepts_unit_test_selectors", FieldDefault::False),
        FieldSpec::new(
            "sql_test_selection",
            FieldDefault::NativeCall(RecordId::SqlTestSelection),
        ),
        FieldSpec::new("source_lexical_syntax", FieldDefault::None),
    ],
};
const R_PYTHON_PLAN_ENTRY: RecordSpec = RecordSpec {
    name: "PythonPlanEntry",
    doc: "Display-ready Python node entry for plan output.",
    fields: &[
        FieldSpec::required("name"),
        FieldSpec::required("kind"),
        FieldSpec::required("phase"),
        FieldSpec::new(
            "identity_status",
            FieldDefault::NativeMember(RecordId::PythonIdentityStatus, "UNKNOWN"),
        ),
        FieldSpec::new("current_definition_json", FieldDefault::None),
        FieldSpec::new("previous_definition_json", FieldDefault::None),
        FieldSpec::new("current_metadata_json", FieldDefault::None),
        FieldSpec::new("previous_metadata_json", FieldDefault::None),
        FieldSpec::new("provider_usages", FieldDefault::EmptyTuple),
    ],
};
const R_PYTHON_RUN_PLAN_OUTPUTS: RecordSpec = RecordSpec {
    name: "PythonRunPlanOutputs",
    doc: "Plan output and Python entries after python-aware run selection.",
    fields: &[
        FieldSpec::required("plan_output"),
        FieldSpec::required("python_plan_entries"),
        FieldSpec::required("selected_python_node_names"),
    ],
};
const R_COMPILED_PROJECT_PHASE_RESULT: RecordSpec = RecordSpec {
    name: "CompiledProjectPhaseResult",
    doc: "Canonical project compilation output before command-specific planning.",
    fields: &[
        FieldSpec::required("project"),
        FieldSpec::required("connection_config"),
        FieldSpec::required("compile_seconds"),
    ],
};
const R_STATIC_COMMAND_CONTEXT: RecordSpec = RecordSpec {
    name: "StaticCommandContext",
    doc: "Shared compile, selection, and relation phases for focused commands.",
    fields: &[
        FieldSpec::required("project"),
        FieldSpec::required("scope"),
        FieldSpec::required("relations"),
        FieldSpec::required("connection_config"),
        FieldSpec::required("compile_seconds"),
    ],
};
const R_CLONE_PIPELINE_OPTIONS: RecordSpec = RecordSpec {
    name: "ClonePipelineOptions",
    doc: "Compilation and selection options for one clone pipeline.",
    fields: &[
        FieldSpec::new("no_sql_validation", FieldDefault::False),
        FieldSpec::new("select", FieldDefault::EmptyTuple),
        FieldSpec::new("exclude", FieldDefault::EmptyTuple),
        FieldSpec::new("cli_vars", FieldDefault::None),
        FieldSpec::new("no_cache", FieldDefault::False),
    ],
};
const R_CLONE_PIPELINE_CONNECTION: RecordSpec = RecordSpec {
    name: "ClonePipelineConnection",
    doc: "Resolved destination connection used throughout clone compilation and planning.",
    fields: &[FieldSpec::required("config"), FieldSpec::required("handle")],
};
const R_CLONE_PIPELINE_RESULT: RecordSpec = RecordSpec {
    name: "ClonePipelineResult",
    doc: "Prepared clone inputs for origin and destination target environments.",
    fields: &[
        FieldSpec::required("origin_project"),
        FieldSpec::required("destination_project"),
        FieldSpec::required("clone_plan"),
        FieldSpec::new("destination_source_entries", FieldDefault::EmptyTuple),
        FieldSpec::new("destination_model_entries", FieldDefault::EmptyTuple),
        FieldSpec::new("destination_seed_entries", FieldDefault::EmptyTuple),
        FieldSpec::new("destination_function_entries", FieldDefault::EmptyTuple),
        FieldSpec::new("origin_model_entries", FieldDefault::EmptyTuple),
        FieldSpec::new("origin_seed_entries", FieldDefault::EmptyTuple),
        FieldSpec::new("origin_source_entries", FieldDefault::EmptyTuple),
    ],
};
const R_DISCOVERED_PYTHON_LOADER_METADATA: RecordSpec = RecordSpec {
    name: "DiscoveredPythonLoaderMetadata",
    doc: "Loader-specific metadata carried by an internal Python loader node.",
    fields: &[
        FieldSpec::new("destination", FieldDefault::None),
        FieldSpec::new("write_strategy", FieldDefault::None),
        FieldSpec::new("cursor_column", FieldDefault::None),
        FieldSpec::new("unique_key", FieldDefault::EmptyTuple),
        FieldSpec::new("columns", FieldDefault::EmptyTuple),
        FieldSpec::new("contract", FieldDefault::None),
        FieldSpec::new(
            "connection_mode",
            FieldDefault::NativeMember(RecordId::LoaderConnectionMode, "SQLBUILD"),
        ),
    ],
};
const R_DISCOVERED_PYTHON_TASK_METADATA: RecordSpec = RecordSpec {
    name: "DiscoveredPythonTaskMetadata",
    doc: "Task-specific metadata carried by an internal Python task node.",
    fields: &[FieldSpec::new("retry", FieldDefault::None)],
};
const R_DISCOVERED_PYTHON_ASSET_METADATA: RecordSpec = RecordSpec {
    name: "DiscoveredPythonAssetMetadata",
    doc: "Asset-specific metadata carried by an internal Python asset node.",
    fields: &[
        FieldSpec::new("columns", FieldDefault::EmptyTuple),
        FieldSpec::new("column_lineage", FieldDefault::None),
        FieldSpec::new("retry", FieldDefault::None),
    ],
};
const R_DISCOVERED_PYTHON_CHECK_METADATA: RecordSpec = RecordSpec {
    name: "DiscoveredPythonCheckMetadata",
    doc: "Check-specific metadata carried by an internal Python check node.",
    fields: &[FieldSpec::new(
        "severity",
        FieldDefault::HostMember(UserCodeHost::PythonCheckSeverity.id(), "ERROR"),
    )],
};
const R_PYTHON_NODE_DEPENDENCY_EDGE: RecordSpec = RecordSpec {
    name: "PythonNodeDependencyEdge",
    doc: "Internal dependency edge between two discovered Python nodes.",
    fields: &[
        FieldSpec::required("upstream_name"),
        FieldSpec::required("downstream_name"),
    ],
};
const R_PYTHON_SQL_RUN_SELECTION: RecordSpec = RecordSpec {
    name: "PythonSqlRunSelection",
    doc: "Unified run-command selection excluding Python checks.",
    fields: &[
        FieldSpec::required("sql_keys"),
        FieldSpec::required("python_node_names"),
    ],
};
const R_LITERAL_SQL_RELATION: RecordSpec = RecordSpec {
    name: "LiteralSqlRelation",
    doc: "A relation named in literal SQL in Python code that matched no project relation.",
    fields: &[
        FieldSpec::required("owner_label"),
        FieldSpec::required("owner_kind"),
        FieldSpec::required("relative_path"),
        FieldSpec::required("line"),
        FieldSpec::required("method"),
        FieldSpec::required("relation"),
    ],
};
const R_SQL_BINDING_DIAGNOSTIC: RecordSpec = RecordSpec {
    name: "SqlBindingDiagnostic",
    doc: "One native binding error or non-blocking runtime-conversion warning.",
    fields: &[
        FieldSpec::required("code"),
        FieldSpec::required("message"),
        FieldSpec::new("line", FieldDefault::None),
        FieldSpec::new("column", FieldDefault::None),
        FieldSpec::new("start", FieldDefault::None),
        FieldSpec::new("end", FieldDefault::None),
        FieldSpec::new("severity", FieldDefault::Text("error")),
    ],
};
const R_SQL_BINDING_RESULT: RecordSpec = RecordSpec {
    name: "SqlBindingResult",
    doc: "Stable result of schema-aware semantic SQL validation.",
    fields: &[FieldSpec::new("diagnostics", FieldDefault::EmptyTuple)],
};
const R_SQL_SCHEMA_VALIDATION_REQUEST: RecordSpec = RecordSpec {
    name: "SqlSchemaValidationRequest",
    doc: "One expanded SQL query and its complete relation schemas.",
    fields: &[
        FieldSpec::required("sql"),
        FieldSpec::required("dialect"),
        FieldSpec::required("schema"),
        FieldSpec::new("known_functions", FieldDefault::EmptyTuple),
        FieldSpec::new("known_types", FieldDefault::EmptyTuple),
        FieldSpec::new("quoted_identifiers_ignore_case", FieldDefault::False),
        FieldSpec {
            name: "catalog",
            default: FieldDefault::None,
            compared: false,
            shown: false,
        },
    ],
};
const R_DBT_CLI_OPTIONS: RecordSpec = RecordSpec {
    name: "DbtCliOptions",
    doc: "Resolved dbt CLI options shared by dbt commands.",
    fields: &[
        FieldSpec::new("project_dir", FieldDefault::None),
        FieldSpec::new("profiles_dir", FieldDefault::None),
        FieldSpec::new("target", FieldDefault::None),
        FieldSpec::new("target_path", FieldDefault::None),
        FieldSpec::new("vars", FieldDefault::None),
        FieldSpec::new("state", FieldDefault::None),
        FieldSpec::new("defer", FieldDefault::False),
    ],
};
const R_NORMALIZED_DBT_PROFILE_CONNECTION: RecordSpec = RecordSpec {
    name: "NormalizedDbtProfileConnection",
    doc: "Rendered dbt profile output normalized for SQLBuild.",
    fields: &[
        FieldSpec::required("adapter"),
        FieldSpec::required("connection"),
        FieldSpec::new("target_schema", FieldDefault::None),
        FieldSpec::new("target_database", FieldDefault::None),
        FieldSpec::new("warnings", FieldDefault::EmptyTuple),
    ],
};
const R_DBT_INIT_PROGRESS_CALLBACKS: RecordSpec = RecordSpec {
    name: "DbtInitProgressCallbacks",
    doc: "Optional progress hooks for `sqb dbt init` phases.",
    fields: &[
        FieldSpec::new("start", FieldDefault::None),
        FieldSpec::new("complete", FieldDefault::None),
    ],
};
const R_DBT_INIT_REQUEST: RecordSpec = RecordSpec {
    name: "DbtInitRequest",
    doc: "Request to initialize a SQLBuild project from a dbt project.",
    fields: &[
        FieldSpec::required("cwd"),
        FieldSpec::required("dbt_project_dir"),
        FieldSpec::required("profiles_dir"),
        FieldSpec::required("profile_name"),
        FieldSpec::required("target_name"),
        FieldSpec::required("sqb_output_dir"),
        FieldSpec::new("dry_run", FieldDefault::False),
        FieldSpec::new("overwrite", FieldDefault::False),
        FieldSpec::new("skip_dbt_debug", FieldDefault::False),
        FieldSpec::new(
            "progress_callbacks",
            FieldDefault::NativeCall(RecordId::DbtInitProgressCallbacks),
        ),
    ],
};
const R_DBT_INIT_RESULT: RecordSpec = RecordSpec {
    name: "DbtInitResult",
    doc: "Result from `sqb dbt init`.",
    fields: &[
        FieldSpec::required("output_dir"),
        FieldSpec::required("project_file"),
        FieldSpec::required("project_name"),
        FieldSpec::required("adapter"),
        FieldSpec::required("target_name"),
        FieldSpec::required("profile_name"),
        FieldSpec::required("toml"),
        FieldSpec::new("warnings", FieldDefault::EmptyTuple),
        FieldSpec::new("dry_run", FieldDefault::False),
    ],
};
const R_DBT_COMMAND_RESULT: RecordSpec = RecordSpec {
    name: "DbtCommandResult",
    doc: "Completed dbt command output.",
    fields: &[
        FieldSpec::required("argv"),
        FieldSpec::required("returncode"),
        FieldSpec::new("stdout", FieldDefault::Text("")),
        FieldSpec::new("stderr", FieldDefault::Text("")),
    ],
};
const R_DBT_NODE_EXECUTION_RESULT: RecordSpec = RecordSpec {
    name: "DbtNodeExecutionResult",
    doc: "One dbt node execution result parsed from JSON logs.",
    fields: &[
        FieldSpec::required("unique_id"),
        FieldSpec::required("resource_type"),
        FieldSpec::required("node_name"),
        FieldSpec::required("status"),
        FieldSpec::required("index"),
        FieldSpec::required("total"),
        FieldSpec::required("execution_time"),
        FieldSpec::new("materialized", FieldDefault::None),
        FieldSpec::new("relation_name", FieldDefault::None),
        FieldSpec::new("database", FieldDefault::None),
        FieldSpec::new("schema", FieldDefault::None),
        FieldSpec::new("node_checksum", FieldDefault::None),
        FieldSpec::new("messages", FieldDefault::EmptyTuple),
    ],
};
const R_DBT_LS_RESULT: RecordSpec = RecordSpec {
    name: "DbtLsResult",
    doc: "Parsed result from one dbt ls invocation.",
    fields: &[FieldSpec::required("nodes"), FieldSpec::required("command")],
};
const R_MEASUREMENT_THRESHOLD_BOUND: RecordSpec = RecordSpec {
    name: "MeasurementThresholdBound",
    doc: "One directional threshold bound for a measurement audit.",
    fields: &[
        FieldSpec::required("operator"),
        FieldSpec::new("limit", FieldDefault::None),
        FieldSpec::new("lower", FieldDefault::None),
        FieldSpec::new("upper", FieldDefault::None),
    ],
};
const R_MEASUREMENT_THRESHOLDS: RecordSpec = RecordSpec {
    name: "MeasurementThresholds",
    doc: "Warning and error policy for a measurement audit.",
    fields: &[
        FieldSpec::new("warn", FieldDefault::None),
        FieldSpec::new("error", FieldDefault::None),
    ],
};
const R_MEASUREMENT_CONTRACT: RecordSpec = RecordSpec {
    name: "MeasurementContract",
    doc: "Columns and sample unit returned by a measurement query.",
    fields: &[
        FieldSpec::required("value_column"),
        FieldSpec::new("sample_count_column", FieldDefault::None),
        FieldSpec::new("sample_unit", FieldDefault::None),
    ],
};
const R_AUDIT_IDENTITY: RecordSpec = RecordSpec {
    name: "AuditIdentity",
    doc: "Stable identity values for one planned audit binding.",
    fields: &[
        FieldSpec::required("binding_key"),
        FieldSpec::required("audit_name"),
        FieldSpec::required("definition_fingerprint"),
        FieldSpec::required("execution_fingerprint"),
        FieldSpec::required("severity"),
        FieldSpec::required("run_scope_phase"),
        FieldSpec::required("attachment_kind"),
        FieldSpec::new("attached_target_name", FieldDefault::None),
        FieldSpec::new("attached_column_name", FieldDefault::None),
        FieldSpec::new("always_run", FieldDefault::False),
    ],
};
const R_AUDIT_GATE_IDENTITY: RecordSpec = RecordSpec {
    name: "AuditGateIdentity",
    doc: "Aggregate identity for a model's runtime audit gate.",
    fields: &[
        FieldSpec::required("binding_set_hash"),
        FieldSpec::required("blocking_set_hash"),
        FieldSpec::required("audits"),
    ],
};
const R_SQL_LOGICAL_TYPE: RecordSpec = RecordSpec {
    name: "SqlLogicalType",
    doc: "Logical type inferred for a normalized SQL value.",
    fields: &[
        FieldSpec::required("kind"),
        FieldSpec::new("element_type", FieldDefault::None),
    ],
};
const R_SQL_VALUE: RecordSpec = RecordSpec {
    name: "SqlValue",
    doc: "One validated, canonical SQL value.",
    fields: &[
        FieldSpec::required("logical_type"),
        FieldSpec::required("value"),
    ],
};
const R_SELECTED_DBT_PROFILE_OUTPUT: RecordSpec = RecordSpec {
    name: "SelectedDbtProfileOutput",
    doc: "Selected dbt profile target output before rendering.",
    fields: &[
        FieldSpec::required("profile_name"),
        FieldSpec::required("target_name"),
        FieldSpec::required("output"),
    ],
};
const R_RESOLVED_DBT_PROFILE_OUTPUT: RecordSpec = RecordSpec {
    name: "ResolvedDbtProfileOutput",
    doc: "Rendered dbt profile target output.",
    fields: &[
        FieldSpec::required("project_dir"),
        FieldSpec::required("profiles_dir"),
        FieldSpec::required("profile_name"),
        FieldSpec::required("target_name"),
        FieldSpec::required("output"),
    ],
};
const R_DISCOVERED_PYTHON_NODE: RecordSpec = RecordSpec {
    name: "DiscoveredPythonNode",
    doc: "Shared internal view of a discovered Python DAG node.",
    fields: &[
        FieldSpec::required("kind"),
        FieldSpec::required("file_path"),
        FieldSpec::required("relative_path"),
        FieldSpec::required("name"),
        FieldSpec::required("function"),
        FieldSpec::new("depends_on", FieldDefault::EmptyTuple),
        FieldSpec::new("sql_deps", FieldDefault::EmptyTuple),
        FieldSpec::new("tags", FieldDefault::EmptyTuple),
        FieldSpec::new("group", FieldDefault::None),
        FieldSpec::new("description", FieldDefault::None),
        FieldSpec::new("meta", FieldDefault::None),
        FieldSpec::new("provider_usages", FieldDefault::EmptyTuple),
        FieldSpec::new("loader", FieldDefault::None),
        FieldSpec::new("task", FieldDefault::None),
        FieldSpec::new("asset", FieldDefault::None),
        FieldSpec::new("check", FieldDefault::None),
        FieldSpec::new("identity", FieldDefault::None),
    ],
};
const R_PYTHON_NODE_GRAPH: RecordSpec = RecordSpec {
    name: "PythonNodeGraph",
    doc: "Internal graph inventory for discovered executable Python nodes.",
    fields: &[
        FieldSpec::required("nodes"),
        FieldSpec::required("dependency_edges"),
        FieldSpec::required("upstream_deps"),
        FieldSpec::required("downstream_deps"),
        FieldSpec::required("tag_index"),
        FieldSpec::required("path_index"),
        FieldSpec::required("nodes_by_name"),
        FieldSpec::required("nodes_by_typed_selector"),
    ],
};
const R_PYTHON_SQL_RUN_LIFECYCLE_PLAN: RecordSpec = RecordSpec {
    name: "PythonSqlRunLifecyclePlan",
    doc: "Lifecycle-aware run classification for SQL resources and Python nodes.",
    fields: &[
        FieldSpec::required("ingress_python_node_names"),
        FieldSpec::required("ingress_loader_names"),
        FieldSpec::required("read_side_python_node_names"),
    ],
};
const R_PYTHON_IDENTITY_DEPENDENCY: RecordSpec = RecordSpec {
    name: "PythonIdentityDependency",
    doc: "One first-party Python source object included in a Python node identity.",
    fields: &[
        FieldSpec::required("kind"),
        FieldSpec::required("module"),
        FieldSpec::required("qualname"),
        FieldSpec::required("source_path"),
        FieldSpec::required("source_hash"),
        FieldSpec::required("source_text"),
    ],
};
const R_PYTHON_NODE_IDENTITY: RecordSpec = RecordSpec {
    name: "PythonNodeIdentity",
    doc: "Read-only identity metadata for a SQLBuild Python node.",
    fields: &[
        FieldSpec::required("node_type"),
        FieldSpec::required("node_name"),
        FieldSpec::required("source_path"),
        FieldSpec::required("source_hash"),
        FieldSpec::required("definition_hash"),
        FieldSpec::required("version_hash"),
        FieldSpec::new("dependencies", FieldDefault::EmptyTuple),
        FieldSpec::new("definition_json", FieldDefault::Text("{}")),
        FieldSpec::new("metadata_json", FieldDefault::Text("{}")),
    ],
};
const R_SQL_LEXICAL_SYNTAX: RecordSpec = RecordSpec {
    name: "SqlLexicalSyntax",
    doc: "Dialect rules for where SQL quoted text and comments end.",
    fields: &[
        FieldSpec::new("backslash_escape_quotes", FieldDefault::EmptyFrozenset),
        FieldSpec::new("escape_string_prefix", FieldDefault::False),
        FieldSpec::new("raw_string_prefix", FieldDefault::False),
        FieldSpec::new("triple_quoted_strings", FieldDefault::False),
        FieldSpec::new("nested_block_comments", FieldDefault::False),
        FieldSpec::new(
            "line_comment_prefixes",
            FieldDefault::TextFrozenset(&["--"]),
        ),
    ],
};
const R_COMPILE_PIPELINE_RESULT: RecordSpec = RecordSpec {
    name: "CompilePipelineResult",
    doc: "Complete output from the compile-and-plan pipeline.",
    fields: &[
        FieldSpec::required("project"),
        FieldSpec::required("plan_output"),
        FieldSpec::new("custom_materializations", FieldDefault::EmptyDict),
        FieldSpec::new("python_node_names", FieldDefault::EmptyFrozenset),
        FieldSpec::new("python_plan_entries", FieldDefault::EmptyTuple),
        FieldSpec::new("compile_seconds", FieldDefault::None),
        FieldSpec::new("planning_seconds", FieldDefault::None),
    ],
};

/// The compiler, lint and config value records.
pub(crate) const COMPILER_RECORDS: &[RecordSpec] = &[
    R_NATIVE_PIVOT_TABLES,
    R_NATIVE_COLUMN_QUERY,
    R_NATIVE_QUERY_COLUMNS,
    R_CONTRACT_FINDING,
    R_CONTRACT_EVIDENCE,
    R_CONTRACT_ADOPTION_RESULT,
    R_CONTRACT_VALIDATION_RESULT,
    R_LINEAGE_GRAPH_VIEWS,
    R_ATTACHED_AUDIT_GATE_EDGE,
    R_HOOK_READ_EDGE,
    R_QUALIFIED_LINEAGE_COLUMN,
    R_COLUMN_LINEAGE_SOURCE,
    R_COLUMN_LINEAGE_EDGE,
    R_DIRECT_SEMANTIC_COLUMN_USE,
    R_COLUMN_LINEAGE,
    R_MODEL_COLUMN_LINEAGE,
    R_REFACTOR_PROJECT,
    R_MANUAL_LOCATION,
    R_NATIVE_SQL_TEST_FAILURE,
    R_NATIVE_SQL_TEST_ASSEMBLY,
    R_HEADER_SPAN,
    R_LINT_FILE_ROLE,
    R_INTERPOLATION_SITE,
    R_LINT_BODY,
    R_LINT_EDIT,
    R_LINT_VIOLATION,
    R_RULE_FIX_RESULT,
    R_LINT_CONFIG,
    R_LINT_RUN_RESULT,
    R_COMPILE_FACTS,
    R_FIX_VERDICT,
    R_DISPLAY_OPTIONS,
    R_COMPILE_PIPELINE_OPTIONS,
    R_PYTHON_PLAN_ENTRY,
    R_PYTHON_RUN_PLAN_OUTPUTS,
    R_COMPILED_PROJECT_PHASE_RESULT,
    R_STATIC_COMMAND_CONTEXT,
    R_CLONE_PIPELINE_OPTIONS,
    R_CLONE_PIPELINE_CONNECTION,
    R_CLONE_PIPELINE_RESULT,
    R_DISCOVERED_PYTHON_LOADER_METADATA,
    R_DISCOVERED_PYTHON_TASK_METADATA,
    R_DISCOVERED_PYTHON_ASSET_METADATA,
    R_DISCOVERED_PYTHON_CHECK_METADATA,
    R_PYTHON_NODE_DEPENDENCY_EDGE,
    R_PYTHON_SQL_RUN_SELECTION,
    R_LITERAL_SQL_RELATION,
    R_SQL_BINDING_DIAGNOSTIC,
    R_SQL_BINDING_RESULT,
    R_SQL_SCHEMA_VALIDATION_REQUEST,
    R_DBT_CLI_OPTIONS,
    R_NORMALIZED_DBT_PROFILE_CONNECTION,
    R_DBT_INIT_PROGRESS_CALLBACKS,
    R_DBT_INIT_REQUEST,
    R_DBT_INIT_RESULT,
    R_DBT_COMMAND_RESULT,
    R_DBT_NODE_EXECUTION_RESULT,
    R_DBT_LS_RESULT,
    R_MEASUREMENT_THRESHOLD_BOUND,
    R_MEASUREMENT_THRESHOLDS,
    R_MEASUREMENT_CONTRACT,
    R_AUDIT_IDENTITY,
    R_AUDIT_GATE_IDENTITY,
    R_SQL_LOGICAL_TYPE,
    R_SQL_VALUE,
    R_SELECTED_DBT_PROFILE_OUTPUT,
    R_RESOLVED_DBT_PROFILE_OUTPUT,
    R_DISCOVERED_PYTHON_NODE,
    R_PYTHON_NODE_GRAPH,
    R_PYTHON_SQL_RUN_LIFECYCLE_PLAN,
    R_PYTHON_IDENTITY_DEPENDENCY,
    R_PYTHON_NODE_IDENTITY,
    R_SQL_LEXICAL_SYNTAX,
    R_COMPILE_PIPELINE_RESULT,
];
