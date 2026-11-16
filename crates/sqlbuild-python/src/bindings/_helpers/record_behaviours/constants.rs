//! Native behaviour of the hosted shapes: computed attributes, methods, ordering and validation.

use crate::bindings::_helpers::freshness::observation::normalize_observation;
use crate::bindings::_helpers::lineage::lineage_facts::{
    facts_eq, facts_getitem, facts_iter, facts_len, facts_projected, facts_resource_name,
};
use crate::bindings::_helpers::record_behaviours::auditing::{
    finish_measurement_contract, finish_threshold_bound, finish_thresholds,
};
use crate::bindings::_helpers::record_behaviours::compiled::{
    build_context_declaration_expansion, diagnostic_is_error, finish_compiler_diagnostic,
    finish_resource_typed, finish_sql_reference, finish_sql_scenario, finish_sql_test,
    input_attached_schema_name, input_relation_key_row, model_dep_rows, model_destination_row,
    model_has_dynamic_columns, model_inferred_rows, model_key_row, model_output_column_count,
    model_relation_key_row_property, model_schema_column_rows, record_reference_rows,
};
use crate::bindings::_helpers::record_behaviours::config::settings_sql_validation;
use crate::bindings::_helpers::record_behaviours::cursors::{
    finish_cursor_bounds, finish_cursor_snapshot, finish_future_evidence, finish_input_evidence,
    finish_input_relation, finish_maximum_evidence, input_relation_runtime_owned,
    snapshot_watermarks_available,
};
use crate::bindings::_helpers::record_behaviours::discovery::{
    constant_logical_type, constant_scalar_type,
};
use crate::bindings::_helpers::record_behaviours::durations::{
    duration_add_to, duration_fixed_seconds, duration_has_calendar_component,
    duration_is_single_unit_in, duration_is_zero, duration_parse, duration_subtract_from,
    duration_total_months, duration_units, finish_aligned_interval,
};
use crate::bindings::_helpers::record_behaviours::error_inits::{
    macro_argument_error_init, macro_argument_error_located, macro_argument_error_shifted,
    rules_config_error_init, sql_test_reference_error_init,
};
use crate::bindings::_helpers::record_behaviours::freshness::{
    finish_freshness_record, freshness_record_identity,
};
use crate::bindings::_helpers::record_behaviours::lexical::{
    finish_lexical_syntax, lexical_cache_key, lexical_native_mapping, lexical_reads_differently,
};
use crate::bindings::_helpers::record_behaviours::macros::{
    context_render_constant, context_render_enum_member, macro_injects_context,
    project_lineage_graph,
};
use crate::bindings::_helpers::record_behaviours::migrations::{
    column_decision_blocks_build, column_decision_label, column_decision_records_event,
    column_decision_renames, column_event_mentions, column_event_renamed, decision_blocks_build,
    decision_checks_compatibility, decision_moves_data, event_mentions, history_new, history_old,
    history_status, relation_identity_property, relation_matches, value_plan_label,
};
use crate::bindings::_helpers::record_behaviours::planner::{
    column_migration_blocks_build, column_migration_origin_missing, finish_cursor_overrides,
    finish_scenario_artifact_kind, materialization_is_table_backed, model_migration_blocks_build,
    model_migration_origin_hidden, model_migration_origin_missing,
    model_migration_storage_transition, model_migration_target_label, old_name_runs_steps,
    overrides_have_untyped, overrides_typed_as, plan_python_source_entries, projection_key,
    rename_hint_message, retention_decreases,
};
use crate::bindings::_helpers::record_behaviours::python_nodes::graph_selected_sql_refs;
use crate::bindings::_helpers::record_behaviours::references::{
    reference_example_call, reference_fixture_cte_prefix, reference_function_name,
    reference_placeholder_call,
};
use crate::bindings::_helpers::record_behaviours::rule_shapes::{
    project_path_depth, project_path_name, project_path_parts, rule_option_boolean,
    rule_option_class_getitem, rule_option_integer, rule_option_integer_list, rule_option_string,
    rule_option_string_list, rule_project_wide, rule_result_finding_count,
};
use crate::bindings::_helpers::record_behaviours::scopes::{
    completeness_complete, target_query_unknown,
};
use crate::bindings::_helpers::record_behaviours::values::{
    finish_fingerprint_set, logical_type_display_name, sql_value_kind,
};
use crate::bindings::_helpers::{
    analysis, analysis_session, build_progress, compile_command, compiled_views, compute_logs,
    dbt_events, dbt_interop, discovery, event_exporting, execution_history, graph, model_config,
    planner_entries, planner_migrations, planner_phases, project_assembly, result_output, rules,
    scopes,
};
use sqlbuild_python_base::records::models::{GetterEntry, RecordBehaviours};
use sqlbuild_python_base::records::types::ClassMethod;
use sqlbuild_python_base::records::types::ClassProperty;
use sqlbuild_python_base::records::types::RecordFinish;

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
    compiled_views::getters::GETTERS,
    compute_logs::getters::GETTERS,
    dbt_events::getters::GETTERS,
    dbt_interop::getters::GETTERS,
    sqlbuild_python_base::diagnostics::constants::GETTERS,
    discovery::getters::GETTERS,
    event_exporting::getters::GETTERS,
    execution_history::getters::GETTERS,
    graph::getters::GETTERS,
    model_config::getters::GETTERS,
    planner_entries::getters::GETTERS,
    planner_migrations::getters::GETTERS,
    planner_phases::getters::GETTERS,
    sqlbuild_python_base::presentation_text::constants::GETTERS,
    project_assembly::getters::GETTERS,
    result_output::getters::GETTERS,
    rules::getters::GETTERS,
    scopes::getters::GETTERS,
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
