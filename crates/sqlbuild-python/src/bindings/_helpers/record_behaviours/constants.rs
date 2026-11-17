//! Native behaviour of the hosted shapes: computed attributes, methods, ordering and validation.

use crate::bindings::_helpers::analysis;
use crate::bindings::_helpers::analysis_session;
use crate::bindings::_helpers::build_progress;
use crate::bindings::_helpers::compile_command;
use crate::bindings::_helpers::compute_logs;
use crate::bindings::_helpers::dbt_events;
use crate::bindings::_helpers::dbt_interop;
use crate::bindings::_helpers::event_exporting;
use crate::bindings::_helpers::execution_history;
use crate::bindings::_helpers::project_assembly;
use crate::bindings::_helpers::record_behaviours::auditing::finish_measurement_contract;
use crate::bindings::_helpers::record_behaviours::auditing::finish_threshold_bound;
use crate::bindings::_helpers::record_behaviours::auditing::finish_thresholds;
use crate::bindings::_helpers::record_behaviours::compiled::build_context_declaration_expansion;
use crate::bindings::_helpers::record_behaviours::compiled::diagnostic_is_error;
use crate::bindings::_helpers::record_behaviours::compiled::finish_compiler_diagnostic;
use crate::bindings::_helpers::record_behaviours::compiled::finish_resource_typed;
use crate::bindings::_helpers::record_behaviours::compiled::finish_sql_reference;
use crate::bindings::_helpers::record_behaviours::compiled::finish_sql_scenario;
use crate::bindings::_helpers::record_behaviours::compiled::finish_sql_test;
use crate::bindings::_helpers::record_behaviours::compiled::input_attached_schema_name;
use crate::bindings::_helpers::record_behaviours::compiled::input_relation_key_row;
use crate::bindings::_helpers::record_behaviours::compiled::model_dep_rows;
use crate::bindings::_helpers::record_behaviours::compiled::model_destination_row;
use crate::bindings::_helpers::record_behaviours::compiled::model_has_dynamic_columns;
use crate::bindings::_helpers::record_behaviours::compiled::model_inferred_rows;
use crate::bindings::_helpers::record_behaviours::compiled::model_key_row;
use crate::bindings::_helpers::record_behaviours::compiled::model_output_column_count;
use crate::bindings::_helpers::record_behaviours::compiled::model_relation_key_row_property;
use crate::bindings::_helpers::record_behaviours::compiled::model_schema_column_rows;
use crate::bindings::_helpers::record_behaviours::compiled::record_reference_rows;
use crate::bindings::_helpers::record_behaviours::config::settings_sql_validation;
use crate::bindings::_helpers::record_behaviours::cursors::finish_cursor_bounds;
use crate::bindings::_helpers::record_behaviours::cursors::finish_cursor_snapshot;
use crate::bindings::_helpers::record_behaviours::cursors::finish_future_evidence;
use crate::bindings::_helpers::record_behaviours::cursors::finish_input_evidence;
use crate::bindings::_helpers::record_behaviours::cursors::finish_input_relation;
use crate::bindings::_helpers::record_behaviours::cursors::finish_maximum_evidence;
use crate::bindings::_helpers::record_behaviours::cursors::input_relation_runtime_owned;
use crate::bindings::_helpers::record_behaviours::cursors::snapshot_watermarks_available;
use crate::bindings::_helpers::record_behaviours::discovery::constant_logical_type;
use crate::bindings::_helpers::record_behaviours::discovery::constant_scalar_type;
use crate::bindings::_helpers::record_behaviours::durations::duration_add_to;
use crate::bindings::_helpers::record_behaviours::durations::duration_fixed_seconds;
use crate::bindings::_helpers::record_behaviours::durations::duration_has_calendar_component;
use crate::bindings::_helpers::record_behaviours::durations::duration_is_single_unit_in;
use crate::bindings::_helpers::record_behaviours::durations::duration_is_zero;
use crate::bindings::_helpers::record_behaviours::durations::duration_parse;
use crate::bindings::_helpers::record_behaviours::durations::duration_subtract_from;
use crate::bindings::_helpers::record_behaviours::durations::duration_total_months;
use crate::bindings::_helpers::record_behaviours::durations::duration_units;
use crate::bindings::_helpers::record_behaviours::durations::finish_aligned_interval;
use crate::bindings::_helpers::record_behaviours::error_inits::macro_argument_error_init;
use crate::bindings::_helpers::record_behaviours::error_inits::macro_argument_error_located;
use crate::bindings::_helpers::record_behaviours::error_inits::macro_argument_error_shifted;
use crate::bindings::_helpers::record_behaviours::error_inits::rules_config_error_init;
use crate::bindings::_helpers::record_behaviours::error_inits::sql_test_reference_error_init;
use crate::bindings::_helpers::record_behaviours::freshness::finish_freshness_record;
use crate::bindings::_helpers::record_behaviours::freshness::freshness_record_identity;
use crate::bindings::_helpers::record_behaviours::lexical::finish_lexical_syntax;
use crate::bindings::_helpers::record_behaviours::lexical::lexical_cache_key;
use crate::bindings::_helpers::record_behaviours::lexical::lexical_native_mapping;
use crate::bindings::_helpers::record_behaviours::lexical::lexical_reads_differently;
use crate::bindings::_helpers::record_behaviours::macros::context_render_constant;
use crate::bindings::_helpers::record_behaviours::macros::context_render_enum_member;
use crate::bindings::_helpers::record_behaviours::macros::macro_injects_context;
use crate::bindings::_helpers::record_behaviours::macros::project_lineage_graph;
use crate::bindings::_helpers::record_behaviours::migrations::column_decision_blocks_build;
use crate::bindings::_helpers::record_behaviours::migrations::column_decision_label;
use crate::bindings::_helpers::record_behaviours::migrations::column_decision_records_event;
use crate::bindings::_helpers::record_behaviours::migrations::column_decision_renames;
use crate::bindings::_helpers::record_behaviours::migrations::column_event_mentions;
use crate::bindings::_helpers::record_behaviours::migrations::column_event_renamed;
use crate::bindings::_helpers::record_behaviours::migrations::decision_blocks_build;
use crate::bindings::_helpers::record_behaviours::migrations::decision_checks_compatibility;
use crate::bindings::_helpers::record_behaviours::migrations::decision_moves_data;
use crate::bindings::_helpers::record_behaviours::migrations::event_mentions;
use crate::bindings::_helpers::record_behaviours::migrations::history_new;
use crate::bindings::_helpers::record_behaviours::migrations::history_old;
use crate::bindings::_helpers::record_behaviours::migrations::history_status;
use crate::bindings::_helpers::record_behaviours::migrations::relation_identity_property;
use crate::bindings::_helpers::record_behaviours::migrations::relation_matches;
use crate::bindings::_helpers::record_behaviours::migrations::value_plan_label;
use crate::bindings::_helpers::record_behaviours::planner::column_migration_blocks_build;
use crate::bindings::_helpers::record_behaviours::planner::column_migration_origin_missing;
use crate::bindings::_helpers::record_behaviours::planner::finish_cursor_overrides;
use crate::bindings::_helpers::record_behaviours::planner::finish_scenario_artifact_kind;
use crate::bindings::_helpers::record_behaviours::planner::materialization_is_table_backed;
use crate::bindings::_helpers::record_behaviours::planner::model_migration_blocks_build;
use crate::bindings::_helpers::record_behaviours::planner::model_migration_origin_hidden;
use crate::bindings::_helpers::record_behaviours::planner::model_migration_origin_missing;
use crate::bindings::_helpers::record_behaviours::planner::model_migration_storage_transition;
use crate::bindings::_helpers::record_behaviours::planner::model_migration_target_label;
use crate::bindings::_helpers::record_behaviours::planner::old_name_runs_steps;
use crate::bindings::_helpers::record_behaviours::planner::overrides_have_untyped;
use crate::bindings::_helpers::record_behaviours::planner::overrides_typed_as;
use crate::bindings::_helpers::record_behaviours::planner::plan_python_source_entries;
use crate::bindings::_helpers::record_behaviours::planner::projection_key;
use crate::bindings::_helpers::record_behaviours::planner::rename_hint_message;
use crate::bindings::_helpers::record_behaviours::planner::retention_decreases;
use crate::bindings::_helpers::record_behaviours::python_nodes::graph_selected_sql_refs;
use crate::bindings::_helpers::record_behaviours::references::reference_example_call;
use crate::bindings::_helpers::record_behaviours::references::reference_fixture_cte_prefix;
use crate::bindings::_helpers::record_behaviours::references::reference_function_name;
use crate::bindings::_helpers::record_behaviours::references::reference_placeholder_call;
use crate::bindings::_helpers::record_behaviours::rule_shapes::project_path_depth;
use crate::bindings::_helpers::record_behaviours::rule_shapes::project_path_name;
use crate::bindings::_helpers::record_behaviours::rule_shapes::project_path_parts;
use crate::bindings::_helpers::record_behaviours::rule_shapes::rule_option_boolean;
use crate::bindings::_helpers::record_behaviours::rule_shapes::rule_option_class_getitem;
use crate::bindings::_helpers::record_behaviours::rule_shapes::rule_option_integer;
use crate::bindings::_helpers::record_behaviours::rule_shapes::rule_option_integer_list;
use crate::bindings::_helpers::record_behaviours::rule_shapes::rule_option_string;
use crate::bindings::_helpers::record_behaviours::rule_shapes::rule_option_string_list;
use crate::bindings::_helpers::record_behaviours::rule_shapes::rule_project_wide;
use crate::bindings::_helpers::record_behaviours::rule_shapes::rule_result_finding_count;
use crate::bindings::_helpers::record_behaviours::scopes::completeness_complete;
use crate::bindings::_helpers::record_behaviours::scopes::target_query_unknown;
use crate::bindings::_helpers::record_behaviours::values::finish_fingerprint_set;
use crate::bindings::_helpers::record_behaviours::values::logical_type_display_name;
use crate::bindings::_helpers::record_behaviours::values::sql_value_kind;
use crate::bindings::_helpers::result_output;
use crate::bindings::_helpers::rules;
use sqlbuild_python_base::records::models::{GetterEntry, RecordBehaviours};
use sqlbuild_python_base::records::types::ClassMethod;
use sqlbuild_python_base::records::types::ClassProperty;
use sqlbuild_python_base::records::types::RecordFinish;
use sqlbuild_python_planner::freshness::main::normalize_observation::normalize_observation;
use sqlbuild_python_project::lineage::main::facts_eq::facts_eq;
use sqlbuild_python_project::lineage::main::facts_getitem::facts_getitem;
use sqlbuild_python_project::lineage::main::facts_iter::facts_iter;
use sqlbuild_python_project::lineage::main::facts_len::facts_len;
use sqlbuild_python_project::lineage::main::facts_projected::facts_projected;
use sqlbuild_python_project::lineage::main::facts_resource_name::facts_resource_name;

/// Compiler records whose bound values are normalized or validated natively.
pub(crate) const BEHAVIOUR_FINISHERS: &[(&str, RecordFinish)] = &[
    ("ScenarioArtifactIdentity", finish_scenario_artifact_kind),
    ("MeasurementThresholdBound", finish_threshold_bound),
    ("MeasurementThresholds", finish_thresholds),
    ("MeasurementContract", finish_measurement_contract),
    ("FingerprintSet", finish_fingerprint_set),
    ("SourceFreshnessRecord", finish_freshness_record),
    ("SqlLexicalSyntax", finish_lexical_syntax),
    ("AlignedInterval", finish_aligned_interval),
    ("CursorOverrides", finish_cursor_overrides),
    ("ModelCursorSnapshot", finish_cursor_snapshot),
    ("CursorBounds", finish_cursor_bounds),
    ("CursorInputEvidence", finish_input_evidence),
    ("FutureCursorSafetyEvidence", finish_future_evidence),
    ("MaximumStartSafetyEvidence", finish_maximum_evidence),
    ("CursorInputRelation", finish_input_relation),
    ("CompilerDiagnostic", finish_compiler_diagnostic),
    ("CompileSqlReference", finish_sql_reference),
    ("CompiledLineageSourceFact", finish_resource_typed),
    ("CompiledObjectKey", finish_resource_typed),
    ("CompiledSqlScenario", finish_sql_scenario),
    ("CompiledSqlTest", finish_sql_test),
];
/// Computed attributes of compiler shapes.
pub(crate) const BEHAVIOUR_PROPERTIES: &[(&str, &str, ClassProperty)] = &[
    ("ColumnRenameHint", "message", rename_hint_message),
    ("QueryProjection", "key", projection_key),
    ("ScopeCompleteness", "complete", completeness_complete),
    ("ScopeTargetQuery", "unknown", target_query_unknown),
    ("SettingsConfig", "sql_validation", settings_sql_validation),
    ("SqlLogicalType", "display_name", logical_type_display_name),
    ("SqlValue", "kind", sql_value_kind),
    (
        "SourceFreshnessRecord",
        "identity",
        freshness_record_identity,
    ),
    ("SqlLexicalSyntax", "cache_key", lexical_cache_key),
    ("SqlLexicalSyntax", "native_mapping", lexical_native_mapping),
    ("Duration", "is_zero", duration_is_zero),
    ("Duration", "units", duration_units),
    (
        "Duration",
        "has_calendar_component",
        duration_has_calendar_component,
    ),
    ("Duration", "fixed_seconds", duration_fixed_seconds),
    ("Duration", "total_months", duration_total_months),
    ("MigrationDecision", "moves_data", decision_moves_data),
    ("MigrationDecision", "blocks_build", decision_blocks_build),
    (
        "MigrationDecision",
        "checks_compatibility",
        decision_checks_compatibility,
    ),
    ("MigrationDecision", "label", value_plan_label),
    ("MigrationPromotion", "label", value_plan_label),
    (
        "ColumnMigrationDecision",
        "renames",
        column_decision_renames,
    ),
    (
        "ColumnMigrationDecision",
        "records_event",
        column_decision_records_event,
    ),
    (
        "ColumnMigrationDecision",
        "blocks_build",
        column_decision_blocks_build,
    ),
    ("ColumnMigrationDecision", "label", column_decision_label),
    ("MigrationRelation", "identity", relation_identity_property),
    ("OldNameViewHistory", "old", history_old),
    ("OldNameViewHistory", "new", history_new),
    ("CursorOverrides", "has_untyped", overrides_have_untyped),
    (
        "ModelCursorSnapshot",
        "watermarks_available",
        snapshot_watermarks_available,
    ),
    (
        "CursorInputRelation",
        "is_runtime_owned",
        input_relation_runtime_owned,
    ),
    ("RetentionPlanEntry", "decreases", retention_decreases),
    (
        "ModelMigrationPlanEntry",
        "origin_missing",
        model_migration_origin_missing,
    ),
    (
        "ModelMigrationPlanEntry",
        "origin_hidden",
        model_migration_origin_hidden,
    ),
    (
        "ModelMigrationPlanEntry",
        "storage_transition",
        model_migration_storage_transition,
    ),
    (
        "ModelMigrationPlanEntry",
        "target_label",
        model_migration_target_label,
    ),
    (
        "ModelMigrationPlanEntry",
        "blocks_build",
        model_migration_blocks_build,
    ),
    ("OldNameViewPlanEntry", "runs_steps", old_name_runs_steps),
    (
        "ColumnMigrationPlanEntry",
        "origin_missing",
        column_migration_origin_missing,
    ),
    (
        "ColumnMigrationPlanEntry",
        "blocks_build",
        column_migration_blocks_build,
    ),
    (
        "PlanOutput",
        "python_source_entries",
        plan_python_source_entries,
    ),
    ("SqlReferenceKind", "function_name", reference_function_name),
    (
        "SqlReferenceKind",
        "fixture_cte_prefix",
        reference_fixture_cte_prefix,
    ),
    ("ConstantDeclaration", "logical_type", constant_logical_type),
    ("ConstantDeclaration", "scalar_type", constant_scalar_type),
    ("CompilerDiagnostic", "is_error", diagnostic_is_error),
    (
        "ModelInputBuildContext",
        "declaration_expansion",
        build_context_declaration_expansion,
    ),
    ("CompileModelInput", "reference_rows", record_reference_rows),
    (
        "CompileModelInput",
        "relation_key_row",
        input_relation_key_row,
    ),
    (
        "CompileModelInput",
        "attached_schema_name",
        input_attached_schema_name,
    ),
    ("CompiledModel", "reference_rows", record_reference_rows),
    (
        "CompiledModel",
        "relation_key_row",
        model_relation_key_row_property,
    ),
    ("CompiledModel", "key_row", model_key_row),
    ("CompiledModel", "dep_rows", model_dep_rows),
    ("CompiledModel", "destination_row", model_destination_row),
    ("CompiledModel", "inferred_rows", model_inferred_rows),
    (
        "CompiledModel",
        "schema_column_rows",
        model_schema_column_rows,
    ),
    (
        "CompiledModel",
        "has_dynamic_columns",
        model_has_dynamic_columns,
    ),
    (
        "CompiledModel",
        "output_column_count",
        model_output_column_count,
    ),
    ("ProjectPath", "parts", project_path_parts),
    ("ProjectPath", "name", project_path_name),
    ("ProjectPath", "depth", project_path_depth),
    ("Rule", "project_wide", rule_project_wide),
    ("RuleResult", "finding_count", rule_result_finding_count),
];
/// Computed attributes of compiler shapes that a record computes once and keeps.
pub(crate) const BEHAVIOUR_CACHED_PROPERTIES: &[(&str, &str, ClassProperty)] = &[
    ("LoadedMacro", "injects_context", macro_injects_context),
    ("CompactLineageFacts", "facts", facts_projected),
    ("CompiledProject", "lineage_graph", project_lineage_graph),
];
/// Methods of compiler shapes.
pub(crate) const BEHAVIOUR_METHODS: &[(&str, &str, ClassMethod)] = &[
    (
        "SqlLexicalSyntax",
        "reads_differently_from_generic",
        lexical_reads_differently,
    ),
    ("Duration", "parse", duration_parse),
    (
        "PythonNodeGraph",
        "selected_sql_refs",
        graph_selected_sql_refs,
    ),
    ("Duration", "is_single_unit_in", duration_is_single_unit_in),
    ("Duration", "add_to", duration_add_to),
    ("MigrationRelation", "matches", relation_matches),
    ("MigrationEvent", "mentions", event_mentions),
    ("ColumnMigrationEvent", "mentions", column_event_mentions),
    ("ColumnMigrationEvent", "renamed", column_event_renamed),
    ("OldNameViewHistory", "status", history_status),
    (
        "MaterializationType",
        "is_table_backed",
        materialization_is_table_backed,
    ),
    ("CursorOverrides", "typed_as", overrides_typed_as),
    ("SqlReferenceKind", "example_call", reference_example_call),
    (
        "SqlReferenceKind",
        "placeholder_call",
        reference_placeholder_call,
    ),
    ("MacroArgumentError", "__init__", macro_argument_error_init),
    (
        "MacroArgumentError",
        "shifted",
        macro_argument_error_shifted,
    ),
    (
        "MacroArgumentError",
        "located",
        macro_argument_error_located,
    ),
    (
        "SqlTestReferenceError",
        "__init__",
        sql_test_reference_error_init,
    ),
    ("RulesConfigError", "__init__", rules_config_error_init),
    ("Duration", "subtract_from", duration_subtract_from),
    ("MacroContext", "render_constant", context_render_constant),
    (
        "MacroContext",
        "render_enum_member",
        context_render_enum_member,
    ),
    ("CompactLineageFacts", "__len__", facts_len),
    ("CompactLineageFacts", "__getitem__", facts_getitem),
    ("CompactLineageFacts", "__iter__", facts_iter),
    ("CompactLineageFacts", "__eq__", facts_eq),
    ("CompactLineageFacts", "resource_name", facts_resource_name),
    ("RuleOption", "__class_getitem__", rule_option_class_getitem),
    ("RuleOption", "boolean", rule_option_boolean),
    ("RuleOption", "integer", rule_option_integer),
    ("RuleOption", "string", rule_option_string),
    ("RuleOption", "string_list", rule_option_string_list),
    ("RuleOption", "integer_list", rule_option_integer_list),
];
/// Compiler records registered as `collections.abc.Sequence` virtual subclasses.
pub(crate) const BEHAVIOUR_SEQUENCES: &[&str] = &["CompactLineageFacts"];
/// Compiler records ordered by their compared fields.
pub(crate) const BEHAVIOUR_ORDERED: &[&str] = &[
    "ResourceIdentity",
    "DeclarationIdentity",
    "OwnershipRoot",
    "ProjectPath",
];

/// The getter tables of every module that defines native pyclasses.
pub(crate) const GETTER_TABLES: &[&[GetterEntry]] = &[
    analysis::getters::GETTERS,
    analysis_session::getters::GETTERS,
    build_progress::getters::GETTERS,
    compile_command::getters::GETTERS,
    sqlbuild_python_base::compile_context::constants::GETTERS,
    sqlbuild_python_project::compiled_views::constants::GETTERS,
    compute_logs::getters::GETTERS,
    dbt_events::getters::GETTERS,
    dbt_interop::getters::GETTERS,
    sqlbuild_python_base::diagnostics::constants::GETTERS,
    sqlbuild_python_front::discovery::constants::GETTERS,
    event_exporting::getters::GETTERS,
    execution_history::getters::GETTERS,
    sqlbuild_python_project::graph::constants::GETTERS,
    sqlbuild_python_front::model_config::constants::GETTERS,
    sqlbuild_python_planner::planner_entries::constants::GETTERS,
    sqlbuild_python_planner::planner_migrations::constants::GETTERS,
    sqlbuild_python_planner::planner_phases::constants::GETTERS,
    sqlbuild_python_base::presentation_text::constants::GETTERS,
    project_assembly::getters::GETTERS,
    result_output::getters::GETTERS,
    rules::getters::GETTERS,
    sqlbuild_python_front::scopes::constants::GETTERS,
];

/// Every record behaviour, installed into `records::behaviours` at registration.
pub(crate) const RECORD_BEHAVIOURS: RecordBehaviours = RecordBehaviours {
    finishers: &[
        &[("SourceFreshnessObservation", normalize_observation)],
        BEHAVIOUR_FINISHERS,
    ],
    properties: &[BEHAVIOUR_PROPERTIES],
    cached_properties: &[BEHAVIOUR_CACHED_PROPERTIES],
    methods: &[BEHAVIOUR_METHODS],
    ordered: &[BEHAVIOUR_ORDERED],
    sequences: &[BEHAVIOUR_SEQUENCES],
};
