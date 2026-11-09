//! Layouts of the planner, migration, cursor and fingerprint value records hosted natively.

use sqlbuild_host::entries::models::ExecutorHost;

use crate::bindings::_helpers::planner_records::defaults::forward_only_backfill;
use crate::bindings::_helpers::records::ids::RecordId;
use crate::bindings::_helpers::records::models::{EnumSpec, FieldDefault, FieldSpec, RecordSpec};

const R_PARSED_SELECTOR: RecordSpec = RecordSpec {
    name: "ParsedSelector",
    doc: "One parsed selector token before graph resolution.",
    fields: &[
        FieldSpec::required("kind"),
        FieldSpec::required("value"),
        FieldSpec::new("upstream", FieldDefault::False),
        FieldSpec::new("downstream", FieldDefault::False),
    ],
};
const R_PATH_SELECTOR: RecordSpec = RecordSpec {
    name: "PathSelector",
    doc: "A directed path selector between two model names with optional endpoint expansion.",
    fields: &[
        FieldSpec::required("start_name"),
        FieldSpec::required("end_name"),
        FieldSpec::new("upstream", FieldDefault::False),
        FieldSpec::new("downstream", FieldDefault::False),
    ],
};
const R_SQL_TEST_SELECTION: RecordSpec = RecordSpec {
    name: "SqlTestSelection",
    doc: "Unit tests selected or excluded by name, beside the tests of the selected resources.",
    fields: &[
        FieldSpec::new("names", FieldDefault::EmptyFrozenset),
        FieldSpec::new("excluded_names", FieldDefault::EmptyFrozenset),
        FieldSpec::new("models_selected", FieldDefault::True),
    ],
};
const R_PLANNER_SCOPE: RecordSpec = RecordSpec {
    name: "PlannerScope",
    doc: "Resolved graph scope for one planner invocation.",
    fields: &[
        FieldSpec::required("upstream_deps"),
        FieldSpec::required("downstream_deps"),
        FieldSpec::required("all_keys"),
        FieldSpec::required("models_by_name"),
        FieldSpec::required("selected_keys"),
        FieldSpec::required("execution_order"),
        FieldSpec::new("user_selected_keys", FieldDefault::EmptyFrozenset),
        FieldSpec::new("python_read_source_names", FieldDefault::EmptyFrozenset),
        FieldSpec::new(
            "sql_test_selection",
            FieldDefault::NativeCall(RecordId::SqlTestSelection),
        ),
        FieldSpec::new("load_sources", FieldDefault::True),
        FieldSpec::new("end_audit_source_keys", FieldDefault::EmptyFrozenset),
    ],
};
const R_SQL_TEST_ASSERTION_STEP: RecordSpec = RecordSpec {
    name: "SqlTestAssertionStep",
    doc: "One zero-row assertion step in a SQL-native unit test.",
    fields: &[
        FieldSpec::required("name"),
        FieldSpec::required("resolved_sql"),
        FieldSpec::new("lifted_ctes", FieldDefault::EmptyTuple),
        FieldSpec::new("comparison_body_sql", FieldDefault::None),
    ],
};
const R_PLANNER_SELECTION: RecordSpec = RecordSpec {
    name: "PlannerSelection",
    doc: "User-facing selection inputs for one planner invocation.",
    fields: &[
        FieldSpec::new("select", FieldDefault::EmptyTuple),
        FieldSpec::new("exclude", FieldDefault::EmptyTuple),
        FieldSpec::new("selected_keys", FieldDefault::None),
        FieldSpec::new("python_read_source_names", FieldDefault::EmptyFrozenset),
        FieldSpec::new(
            "sql_test_selection",
            FieldDefault::NativeCall(RecordId::SqlTestSelection),
        ),
    ],
};
const R_PLANNER_OVERRIDES: RecordSpec = RecordSpec {
    name: "PlannerOverrides",
    doc: "Explicit user overrides that force or bound planner decisions.",
    fields: &[
        FieldSpec::new("cursor_overrides", FieldDefault::None),
        FieldSpec::new("full_refresh", FieldDefault::False),
        FieldSpec::new("reload_sources", FieldDefault::False),
        FieldSpec::new("forced_stale_model_names", FieldDefault::EmptyTuple),
        FieldSpec::new("external_blocked_model_names", FieldDefault::EmptyTuple),
        FieldSpec::new("max_microbatches", FieldDefault::None),
        FieldSpec::new("concurrency", FieldDefault::None),
    ],
};
const R_DEFERRAL_INPUTS: RecordSpec = RecordSpec {
    name: "DeferralInputs",
    doc: "Deferred relation and source-deferral inputs for planning.",
    fields: &[
        FieldSpec::new("deferred_locations", FieldDefault::None),
        FieldSpec::new("deferred_relations", FieldDefault::None),
        FieldSpec::new("defer_sources_to", FieldDefault::None),
        FieldSpec::new("source_deferral_enabled", FieldDefault::True),
    ],
};
const R_PLANNER_POLICIES: RecordSpec = RecordSpec {
    name: "PlannerPolicies",
    doc: "Behavior policies selected by the caller for one planner invocation.",
    fields: &[
        FieldSpec::new("auto_load_sources", FieldDefault::False),
        FieldSpec::new("selection_diagnostics", FieldDefault::False),
        FieldSpec::new("plan_sql_tests", FieldDefault::True),
        FieldSpec::new("record_migration_fingerprints", FieldDefault::True),
    ],
};
const R_DIRECT_SOURCE_FRESHNESS_PLANNING_RESULT: RecordSpec = RecordSpec {
    name: "DirectSourceFreshnessPlanningResult",
    doc: "Direct planning-time source freshness observations and comparisons.",
    fields: &[
        FieldSpec::new("observed_records", FieldDefault::EmptyTuple),
        FieldSpec::new("previous_records", FieldDefault::EmptyTuple),
        FieldSpec::new("changed_identities", FieldDefault::EmptyFrozenset),
        FieldSpec::new("unchanged_identities", FieldDefault::EmptyFrozenset),
        FieldSpec::new("unknown_source_names", FieldDefault::EmptyTuple),
        FieldSpec::new("unknown_sources", FieldDefault::EmptyDict),
        FieldSpec::new("age_statuses", FieldDefault::EmptyDict),
        FieldSpec::new("propagation", FieldDefault::None),
    ],
};
const R_FINGERPRINT: RecordSpec = RecordSpec {
    name: "Fingerprint",
    doc: "One applied fingerprint record from a successful materialization.",
    fields: &[
        FieldSpec::required("node_type"),
        FieldSpec::required("node_name"),
        FieldSpec::required("target_database"),
        FieldSpec::required("target_schema"),
        FieldSpec::required("target_name"),
        FieldSpec::required("run_id"),
        FieldSpec::required("definition_hash"),
        FieldSpec::required("schema_fingerprint"),
        FieldSpec::required("definition"),
        FieldSpec::required("ts"),
        FieldSpec::new("metadata_json", FieldDefault::Text("{}")),
        FieldSpec::new("version_hash", FieldDefault::Text("")),
    ],
};
const R_FINGERPRINT_SET: RecordSpec = RecordSpec {
    name: "FingerprintSet",
    doc: "Latest fingerprints per node name for one target schema.",
    fields: &[
        FieldSpec::required("schema"),
        FieldSpec::required("fingerprints"),
        FieldSpec::new("fingerprints_by_identity", FieldDefault::None),
    ],
};
const R_SOURCE_FRESHNESS_RECORD: RecordSpec = RecordSpec {
    name: "SourceFreshnessRecord",
    doc: "One append-only direct observation whose observed time is always aware UTC.",
    fields: &[
        FieldSpec::required("source_name"),
        FieldSpec::required("target_database"),
        FieldSpec::required("target_schema"),
        FieldSpec::required("target_name"),
        FieldSpec::required("run_id"),
        FieldSpec::required("strategy"),
        FieldSpec::required("value_kind"),
        FieldSpec::required("data_version"),
        FieldSpec::required("data_version_hash"),
        FieldSpec::required("observed_at"),
    ],
};
const R_TIMESTAMP_VALUE: RecordSpec = RecordSpec {
    name: "TimestampValue",
    doc: "Timestamp cursor value; aware datetimes are normalized to UTC at ingestion.",
    fields: &[
        FieldSpec::required("value"),
        FieldSpec {
            name: "source_text",
            default: FieldDefault::None,
            compared: false,
            shown: false,
        },
    ],
};
const R_DATE_VALUE: RecordSpec = RecordSpec {
    name: "DateValue",
    doc: "Date cursor value which must remain date-formatted when rendered.",
    fields: &[FieldSpec::required("value")],
};
const R_INTEGER_VALUE: RecordSpec = RecordSpec {
    name: "IntegerValue",
    doc: "Integral cursor value.",
    fields: &[FieldSpec::required("value")],
};
const R_ALIGNED_INTERVAL: RecordSpec = RecordSpec {
    name: "AlignedInterval",
    doc: "Validated half-open cursor interval.",
    fields: &[
        FieldSpec::required("start"),
        FieldSpec::required("end"),
        FieldSpec::required("grain"),
    ],
};
const R_DURATION: RecordSpec = RecordSpec {
    name: "Duration",
    doc: "A calendar-aware duration (years/months plus fixed time units).",
    fields: &[
        FieldSpec::new("years", FieldDefault::Int(0)),
        FieldSpec::new("months", FieldDefault::Int(0)),
        FieldSpec::new("days", FieldDefault::Int(0)),
        FieldSpec::new("hours", FieldDefault::Int(0)),
        FieldSpec::new("minutes", FieldDefault::Int(0)),
        FieldSpec::new("seconds", FieldDefault::Int(0)),
    ],
};
const R_MIGRATION_RELATION: RecordSpec = RecordSpec {
    name: "MigrationRelation",
    doc: "One physical relation named by a migration event.",
    fields: &[
        FieldSpec::required("database"),
        FieldSpec::required("schema"),
        FieldSpec::required("name"),
    ],
};
const R_MIGRATION_EVENT: RecordSpec = RecordSpec {
    name: "MigrationEvent",
    doc: "One immutable fact that a origin relation was cloned over a destination.",
    fields: &[
        FieldSpec::required("event_id"),
        FieldSpec::required("target_name"),
        FieldSpec::required("origin_model"),
        FieldSpec::required("origin"),
        FieldSpec::required("destination_model"),
        FieldSpec::required("destination"),
        FieldSpec::required("origin_version_hash"),
        FieldSpec::required("discovery"),
        FieldSpec::required("decision"),
        FieldSpec::required("run_id"),
        FieldSpec::required("created_at"),
    ],
};
const R_COLUMN_MIGRATION_EVENT: RecordSpec = RecordSpec {
    name: "ColumnMigrationEvent",
    doc: "One immutable fact that a column of a model relation was renamed in place.",
    fields: &[
        FieldSpec::required("event_id"),
        FieldSpec::required("target_name"),
        FieldSpec::required("model_name"),
        FieldSpec::required("relation"),
        FieldSpec::required("origin_column"),
        FieldSpec::required("destination_column"),
        FieldSpec::required("discovery"),
        FieldSpec::required("decision"),
        FieldSpec::required("run_id"),
        FieldSpec::required("created_at"),
    ],
};
const R_OLD_NAME_VIEW_EVENT: RecordSpec = RecordSpec {
    name: "OldNameViewEvent",
    doc: "One immutable fact about the old name of one recorded move.",
    fields: &[
        FieldSpec::required("event_id"),
        FieldSpec::required("target_name"),
        FieldSpec::required("event_type"),
        FieldSpec::required("migration_event_id"),
        FieldSpec::required("destination_model"),
        FieldSpec::required("old"),
        FieldSpec::required("new"),
        FieldSpec::required("run_id"),
        FieldSpec::required("created_at"),
        FieldSpec::new("view_retention", FieldDefault::None),
        FieldSpec::new("archive_name", FieldDefault::None),
        FieldSpec::new("column_aliases", FieldDefault::EmptyTuple),
        FieldSpec::new("expires_at", FieldDefault::None),
        FieldSpec::new("drop_reason", FieldDefault::None),
        FieldSpec::new("grants_copied", FieldDefault::None),
        FieldSpec::new("view_sql", FieldDefault::None),
    ],
};
const R_OLD_NAME_VIEW_HISTORY: RecordSpec = RecordSpec {
    name: "OldNameViewHistory",
    doc: "The recorded move and every old-name fact that references it.",
    fields: &[
        FieldSpec::required("move"),
        FieldSpec::required("required"),
        FieldSpec::new("archived", FieldDefault::None),
        FieldSpec::new("created", FieldDefault::None),
        FieldSpec::new("dropped", FieldDefault::None),
    ],
};
const R_CURSOR_OVERRIDES: RecordSpec = RecordSpec {
    name: "CursorOverrides",
    doc: "Cursor override values from CLI flags; untyped `start`/`end` await planner typing.",
    fields: &[
        FieldSpec::new("start_ts", FieldDefault::None),
        FieldSpec::new("end_ts", FieldDefault::None),
        FieldSpec::new("start_int", FieldDefault::None),
        FieldSpec::new("end_int", FieldDefault::None),
        FieldSpec::new("start", FieldDefault::None),
        FieldSpec::new("end", FieldDefault::None),
    ],
};
const R_EMPTY_CURSOR_INPUT_DECISION: RecordSpec = RecordSpec {
    name: "EmptyCursorInputDecision",
    doc: "Cursor inputs whose lack of rows leaves no window, and whether other inputs have rows.",
    fields: &[
        FieldSpec::new("input_names", FieldDefault::EmptyTuple),
        FieldSpec::new("waiting_on_empty_inputs", FieldDefault::False),
    ],
};
const R_MODEL_CURSOR_SNAPSHOT: RecordSpec = RecordSpec {
    name: "ModelCursorSnapshot",
    doc: "Cursor MIN/MAX values gathered from warehouse for one incremental model.",
    fields: &[
        FieldSpec::required("target_max"),
        FieldSpec::required("upstream_mins"),
        FieldSpec::required("upstream_maxes"),
        FieldSpec {
            name: "physical_target_max",
            default: FieldDefault::None,
            compared: false,
            shown: false,
        },
        FieldSpec::new("target_eligible_max", FieldDefault::None),
        FieldSpec {
            name: "target_relation",
            default: FieldDefault::None,
            compared: false,
            shown: false,
        },
        FieldSpec {
            name: "destination_cursor_column",
            default: FieldDefault::None,
            compared: false,
            shown: false,
        },
        FieldSpec {
            name: "input_evidence",
            default: FieldDefault::EmptyTuple,
            compared: false,
            shown: false,
        },
        FieldSpec {
            name: "expected_watermark_count",
            default: FieldDefault::Int(0),
            compared: false,
            shown: false,
        },
        FieldSpec::new("unavailable_watermark_tags", FieldDefault::EmptyTuple),
        FieldSpec::new(
            "cursor_watermark_mode",
            FieldDefault::NativeMember(RecordId::CursorWatermarkMode, "ALL"),
        ),
        FieldSpec::new("upstream_terminal_starts", FieldDefault::EmptyTuple),
        FieldSpec::new("upstream_terminal_ends", FieldDefault::EmptyTuple),
        FieldSpec::new("upstream_end_inputs", FieldDefault::EmptyTuple),
        FieldSpec::new("upstream_availability_ends", FieldDefault::EmptyTuple),
        FieldSpec {
            name: "empty_input_names",
            default: FieldDefault::EmptyTuple,
            compared: false,
            shown: false,
        },
        FieldSpec {
            name: "unreadable_input_names",
            default: FieldDefault::EmptyTuple,
            compared: false,
            shown: false,
        },
    ],
};
const R_CURSOR_BOUNDS: RecordSpec = RecordSpec {
    name: "CursorBounds",
    doc: "Effective cursor start and end values for one incremental model.",
    fields: &[
        FieldSpec::required("start"),
        FieldSpec::required("end"),
        FieldSpec::new("future_safety", FieldDefault::None),
        FieldSpec::new("maximum_start_safety", FieldDefault::None),
    ],
};
const R_CURSOR_INPUT_EVIDENCE: RecordSpec = RecordSpec {
    name: "CursorInputEvidence",
    doc: "Observed bounds for one physical cursor input.",
    fields: &[
        FieldSpec::required("relation"),
        FieldSpec::required("cursor_column"),
        FieldSpec::required("minimum"),
        FieldSpec::required("maximum"),
    ],
};
const R_FUTURE_CURSOR_SAFETY_EVIDENCE: RecordSpec = RecordSpec {
    name: "FutureCursorSafetyEvidence",
    doc: "Structured evidence for one applied future-cursor cap.",
    fields: &[
        FieldSpec::required("action"),
        FieldSpec::required("max_distance"),
        FieldSpec::required("invocation_time"),
        FieldSpec::required("discovered_start"),
        FieldSpec::required("discovered_end"),
        FieldSpec::required("applied_start"),
        FieldSpec::required("applied_end"),
        FieldSpec::required("maximum_allowed_start"),
        FieldSpec::required("maximum_allowed_end"),
        FieldSpec::required("future_start_detected"),
        FieldSpec::required("future_end_detected"),
        FieldSpec::required("determining_relation"),
        FieldSpec::required("determining_cursor_column"),
        FieldSpec::new("inputs", FieldDefault::EmptyTuple),
    ],
};
const R_MAXIMUM_START_SAFETY_EVIDENCE: RecordSpec = RecordSpec {
    name: "MaximumStartSafetyEvidence",
    doc: "Structured evidence for an automatic-start eligibility decision.",
    fields: &[
        FieldSpec::required("action"),
        FieldSpec::required("max_ahead"),
        FieldSpec::required("invocation_time"),
        FieldSpec::required("physical_target_max"),
        FieldSpec::required("highest_eligible_target_max"),
        FieldSpec::required("effective_start"),
        FieldSpec::required("maximum_allowed_start"),
        FieldSpec::required("target_relation"),
        FieldSpec::required("cursor_column"),
    ],
};
const R_CURSOR_INPUT_RELATION: RecordSpec = RecordSpec {
    name: "CursorInputRelation",
    doc: "One cursor-bearing input relation for runtime range discovery.",
    fields: &[
        FieldSpec::required("relation"),
        FieldSpec::required("cursor_column"),
        FieldSpec::new("cursor_grain", FieldDefault::None),
        FieldSpec::new("is_model_backed", FieldDefault::False),
        FieldSpec::new("is_runtime_produced", FieldDefault::False),
        FieldSpec::new("terminal_cursor_start", FieldDefault::None),
        FieldSpec::new("terminal_cursor_end", FieldDefault::None),
        FieldSpec::new("input_name", FieldDefault::None),
        FieldSpec::new("input_kind", FieldDefault::None),
    ],
};
const R_MAXIMUM_START_POLICY_INPUTS: RecordSpec = RecordSpec {
    name: "MaximumStartPolicyInputs",
    doc: "Effective policy and materialization safety inputs for automatic starts.",
    fields: &[
        FieldSpec::new("config", FieldDefault::None),
        FieldSpec::new("invocation_time", FieldDefault::None),
        FieldSpec::new("incremental_strategy", FieldDefault::None),
        FieldSpec::new("incremental_mode", FieldDefault::None),
    ],
};
const R_SCHEMA_FINDING: RecordSpec = RecordSpec {
    name: "SchemaFinding",
    doc: "One detected schema difference between expected and warehouse columns.",
    fields: &[
        FieldSpec::required("kind"),
        FieldSpec::required("column_name"),
        FieldSpec::required("source"),
        FieldSpec::new("expected_type", FieldDefault::None),
        FieldSpec::new("actual_type", FieldDefault::None),
    ],
};
const R_RETENTION_PLAN_ENTRY: RecordSpec = RecordSpec {
    name: "RetentionPlanEntry",
    doc: "Warehouse retention work independent of model identity actions.",
    fields: &[
        FieldSpec::required("request"),
        FieldSpec::required("model_names"),
        FieldSpec::required("actual_days"),
        FieldSpec::required("effective_days"),
        FieldSpec::required("source"),
        FieldSpec::required("direction"),
        FieldSpec::required("phase"),
        FieldSpec::new("statements", FieldDefault::EmptyTuple),
        FieldSpec::new("irreversible_warning", FieldDefault::None),
        FieldSpec::new("decrease_policy", FieldDefault::Text("deny")),
    ],
};
const R_TABLE_TYPE_PLAN_ENTRY: RecordSpec = RecordSpec {
    name: "TableTypePlanEntry",
    doc: "Snowflake table-type work independent of model identity actions.",
    fields: &[
        FieldSpec::required("model_name"),
        FieldSpec::required("destination"),
        FieldSpec::required("copy_name"),
        FieldSpec::required("desired_type"),
        FieldSpec::required("actual_type"),
        FieldSpec::required("source"),
        FieldSpec::required("downgrade"),
        FieldSpec::required("downgrade_policy"),
        FieldSpec::new("irreversible_warning", FieldDefault::None),
    ],
};
const R_MODEL_MIGRATION_PLAN_ENTRY: RecordSpec = RecordSpec {
    name: "ModelMigrationPlanEntry",
    doc: "One declared or discovered model migration and its per-run decision.",
    fields: &[
        FieldSpec::required("model_name"),
        FieldSpec::required("discovery"),
        FieldSpec::required("decision"),
        FieldSpec::required("compatibility"),
        FieldSpec::required("origin_model"),
        FieldSpec::required("origin"),
        FieldSpec::required("destination"),
        FieldSpec::required("target_name"),
        FieldSpec::new("origin_version_hash", FieldDefault::Text("")),
        FieldSpec::new("origin_is_transient", FieldDefault::False),
        FieldSpec::new("stage_is_transient", FieldDefault::None),
        FieldSpec::new("transfer", FieldDefault::None),
        FieldSpec::new("transfer_fallback", FieldDefault::None),
        FieldSpec::new("promotion", FieldDefault::None),
        FieldSpec::new("completed_at", FieldDefault::None),
        FieldSpec::new("compatibility_findings", FieldDefault::EmptyTuple),
        FieldSpec::new("message", FieldDefault::None),
        FieldSpec::new("origin_tracked", FieldDefault::False),
        FieldSpec::new(
            "missing_origin_policy",
            FieldDefault::NativeMember(RecordId::MissingMigrationOriginPolicy, "ALLOW"),
        ),
    ],
};
const R_OLD_NAME_VIEW_PLAN_ENTRY: RecordSpec = RecordSpec {
    name: "OldNameViewPlanEntry",
    doc: "What one build does at the old name of one migrated model.",
    fields: &[
        FieldSpec::required("model_name"),
        FieldSpec::required("origin"),
        FieldSpec::required("destination"),
        FieldSpec::required("action"),
        FieldSpec::required("target_name"),
        FieldSpec::new("retention", FieldDefault::None),
        FieldSpec::new("expires_at", FieldDefault::None),
        FieldSpec::new("column_aliases", FieldDefault::EmptyTuple),
        FieldSpec::new("stores_history", FieldDefault::False),
        FieldSpec::new("migration_event_id", FieldDefault::None),
        FieldSpec::new("records_requirement", FieldDefault::False),
        FieldSpec::new("reason", FieldDefault::None),
        FieldSpec::new("grants_copied", FieldDefault::None),
        FieldSpec::new("grants_supported", FieldDefault::False),
        FieldSpec::new("archived", FieldDefault::False),
    ],
};
const R_OLD_NAME_VIEW: RecordSpec = RecordSpec {
    name: "OldNameView",
    doc: "One compatibility view recorded at an old name and not recorded as dropped.",
    fields: &[
        FieldSpec::required("destination_model"),
        FieldSpec::required("old"),
        FieldSpec::required("new"),
        FieldSpec::required("expires_at"),
        FieldSpec::required("column_aliases"),
        FieldSpec::required("migration_event_id"),
        FieldSpec::required("target_name"),
        FieldSpec::new("name_reused_by", FieldDefault::None),
    ],
};
const R_COLUMN_MIGRATION_PLAN_ENTRY: RecordSpec = RecordSpec {
    name: "ColumnMigrationPlanEntry",
    doc: "One declared or detected in-place column rename and its per-run decision.",
    fields: &[
        FieldSpec::required("model_name"),
        FieldSpec::required("destination"),
        FieldSpec::required("origin_column"),
        FieldSpec::required("destination_column"),
        FieldSpec::required("discovery"),
        FieldSpec::required("decision"),
        FieldSpec::required("target_name"),
        FieldSpec::new("completed_at", FieldDefault::None),
        FieldSpec::new("message", FieldDefault::None),
        FieldSpec::new(
            "missing_origin_policy",
            FieldDefault::NativeMember(RecordId::MissingMigrationOriginPolicy, "ALLOW"),
        ),
    ],
};
const R_MODEL_PLAN_ENTRY: RecordSpec = RecordSpec {
    name: "ModelPlanEntry",
    doc: "Per-model execution plan entry with action, reason, and resolved artifacts.",
    fields: &[
        FieldSpec::required("key"),
        FieldSpec::required("name"),
        FieldSpec::required("relative_path"),
        FieldSpec::required("materialization_type"),
        FieldSpec::required("action"),
        FieldSpec::required("reason"),
        FieldSpec::required("destination"),
        FieldSpec::required("fingerprint_query_sql"),
        FieldSpec::required("resolved_sql"),
        FieldSpec::required("logical_ddl"),
        FieldSpec::new("migration_fingerprint", FieldDefault::None),
        FieldSpec::new("incremental_strategy", FieldDefault::None),
        FieldSpec::new("incremental_mode", FieldDefault::None),
        FieldSpec::new("microbatch_strategy", FieldDefault::None),
        FieldSpec::new("cursor_watermark_mode", FieldDefault::None),
        FieldSpec::new("cursor_column", FieldDefault::None),
        FieldSpec::new("cursor_type", FieldDefault::None),
        FieldSpec::new("cursor_grain", FieldDefault::None),
        FieldSpec::new("cursor_start", FieldDefault::None),
        FieldSpec::new("cursor_end", FieldDefault::None),
        FieldSpec::new("lookback", FieldDefault::None),
        FieldSpec::new("cursor_bounds", FieldDefault::None),
        FieldSpec::new("empty_cursor_inputs", FieldDefault::EmptyTuple),
        FieldSpec::new("waiting_on_empty_inputs", FieldDefault::False),
        FieldSpec::new("cursor_input_relations", FieldDefault::EmptyTuple),
        FieldSpec::new("batch_size", FieldDefault::None),
        FieldSpec::new("batch_concurrency", FieldDefault::Int(1)),
        FieldSpec::new("unaccounted_partition_policy", FieldDefault::None),
        FieldSpec::new("microbatch_range", FieldDefault::None),
        FieldSpec::new("microbatch_limit", FieldDefault::None),
        FieldSpec::new("declared_microbatch_limit_action", FieldDefault::None),
        FieldSpec::new("microbatch_safety_limit", FieldDefault::None),
        FieldSpec::new("microbatch_safety_limit_action", FieldDefault::None),
        FieldSpec::new("microbatch_limit_count", FieldDefault::None),
        FieldSpec::new("microbatch_limit_action", FieldDefault::None),
        FieldSpec::new("microbatch_limit_warning", FieldDefault::None),
        FieldSpec::new("start_cursor_override", FieldDefault::None),
        FieldSpec::new("end_cursor_override", FieldDefault::None),
        FieldSpec::new("future_cursor_config", FieldDefault::None),
        FieldSpec::new("start_cursor_config", FieldDefault::None),
        FieldSpec::new("invocation_time", FieldDefault::None),
        FieldSpec::new("unique_key", FieldDefault::EmptyTuple),
        FieldSpec::new("merge_exclude_columns", FieldDefault::EmptyTuple),
        FieldSpec::new("snapshot_strategy", FieldDefault::None),
        FieldSpec::new("updated_at_column", FieldDefault::None),
        FieldSpec::new("check_columns", FieldDefault::EmptyTuple),
        FieldSpec::new("observed_at_column", FieldDefault::None),
        FieldSpec::new("historical_input", FieldDefault::None),
        FieldSpec::new("valid_from_column", FieldDefault::None),
        FieldSpec::new("valid_to_column", FieldDefault::None),
        FieldSpec::new("initial_valid_from", FieldDefault::None),
        FieldSpec::new("invalidate_hard_deletes", FieldDefault::False),
        FieldSpec::new("snapshot_full_refresh", FieldDefault::None),
        FieldSpec::new("snapshot_schema_change", FieldDefault::None),
        FieldSpec::new("table_type", FieldDefault::Text("transient")),
        FieldSpec::new("physical_storage", FieldDefault::EmptyDict),
        FieldSpec::new("expected_output_columns", FieldDefault::None),
        FieldSpec::new("on_schema_change", FieldDefault::None),
        FieldSpec::new("type_enforcement", FieldDefault::False),
        FieldSpec::new("declared_columns", FieldDefault::EmptyTuple),
        FieldSpec::new("inferred_columns", FieldDefault::EmptyTuple),
        FieldSpec::new("contract_enforced", FieldDefault::False),
        FieldSpec::new("contract_columns", FieldDefault::EmptyTuple),
        FieldSpec::new("contract_dynamic_columns", FieldDefault::EmptyTuple),
        FieldSpec::new("pre_hooks", FieldDefault::None),
        FieldSpec::new("post_hooks", FieldDefault::None),
        FieldSpec::new("previous_query_sql", FieldDefault::None),
        FieldSpec::new("previous_fingerprint", FieldDefault::None),
        FieldSpec::new("fingerprint_metadata_json", FieldDefault::None),
        FieldSpec::new("previous_metadata_json", FieldDefault::None),
        FieldSpec::new("fingerprint_version_hash", FieldDefault::None),
        FieldSpec::new("previous_version_hash", FieldDefault::None),
        FieldSpec::new("fingerprint_definition_hash", FieldDefault::None),
        FieldSpec::new("query_changed", FieldDefault::False),
        FieldSpec::new("config_changed", FieldDefault::False),
        FieldSpec::new("schema_actions", FieldDefault::EmptyTuple),
        FieldSpec::new("schema_findings", FieldDefault::EmptyTuple),
        FieldSpec::new("backfill", FieldDefault::Factory(forward_only_backfill)),
        FieldSpec::new("changed_functions", FieldDefault::EmptyTuple),
        FieldSpec::new("custom_materialization_name", FieldDefault::None),
        FieldSpec::new("custom_config", FieldDefault::EmptyDict),
        FieldSpec::new("custom_placeholders", FieldDefault::EmptyDict),
        FieldSpec::new("column_rename_hints", FieldDefault::EmptyTuple),
        FieldSpec::new("max_duration", FieldDefault::None),
        FieldSpec::new("max_duration_seconds", FieldDefault::None),
        FieldSpec::new("max_duration_source", FieldDefault::None),
    ],
};
const R_CLONE_SOURCE_PLAN_ENTRY: RecordSpec = RecordSpec {
    name: "CloneSourcePlanEntry",
    doc: "One managed source relation selected for target cloning.",
    fields: &[
        FieldSpec::required("key"),
        FieldSpec::required("name"),
        FieldSpec::required("destination"),
    ],
};
const R_SEED_PLAN_ENTRY: RecordSpec = RecordSpec {
    name: "SeedPlanEntry",
    doc: "Per-seed execution plan entry.",
    fields: &[
        FieldSpec::required("key"),
        FieldSpec::required("name"),
        FieldSpec::required("destination"),
        FieldSpec::required("file_path"),
        FieldSpec::required("columns"),
        FieldSpec::required("csv_settings"),
        FieldSpec::new("fingerprint_definition", FieldDefault::Text("")),
        FieldSpec::new("fingerprint_version_hash", FieldDefault::Text("")),
        FieldSpec::new("fingerprint_metadata_json", FieldDefault::Text("{}")),
        FieldSpec::new(
            "action",
            FieldDefault::NativeMember(RecordId::PlanAction, "LOAD_SEED"),
        ),
        FieldSpec::new(
            "reason",
            FieldDefault::NativeMember(RecordId::PlanReason, "FIRST_RUN"),
        ),
    ],
};
const R_SOURCE_LOAD_PLAN_ENTRY: RecordSpec = RecordSpec {
    name: "SourceLoadPlanEntry",
    doc: "Per-managed-source loader execution plan entry.",
    fields: &[
        FieldSpec::required("key"),
        FieldSpec::required("name"),
        FieldSpec::required("loader"),
        FieldSpec::required("destination"),
        FieldSpec::new(
            "resource_kind",
            FieldDefault::HostMember(ExecutorHost::ExecutionResourceKind.id(), "SOURCE"),
        ),
        FieldSpec::new("write_strategy", FieldDefault::None),
        FieldSpec::new("cursor_column", FieldDefault::None),
        FieldSpec::new("unique_key", FieldDefault::EmptyTuple),
        FieldSpec::new("is_reload", FieldDefault::False),
        FieldSpec::new("integration_kind", FieldDefault::None),
    ],
};
const R_FUNCTION_PLAN_ENTRY: RecordSpec = RecordSpec {
    name: "FunctionPlanEntry",
    doc: "Per-SQL-function execution plan entry.",
    fields: &[
        FieldSpec::required("key"),
        FieldSpec::required("name"),
        FieldSpec::required("relative_path"),
        FieldSpec::required("destination"),
        FieldSpec::required("arguments"),
        FieldSpec::required("returns"),
        FieldSpec::required("body_sql"),
        FieldSpec::required("fingerprint_query_sql"),
        FieldSpec::required("fingerprint_destination"),
        FieldSpec::new("return_columns", FieldDefault::EmptyTuple),
        FieldSpec::new(
            "language",
            FieldDefault::Member("sqlbuild.compiler.compile.types", "FunctionLanguage", "SQL"),
        ),
        FieldSpec::new("source_file_path", FieldDefault::None),
        FieldSpec::new("runtime_version", FieldDefault::None),
        FieldSpec::new("entry_point", FieldDefault::None),
        FieldSpec::new("packages", FieldDefault::EmptyTuple),
        FieldSpec::new("previous_query_sql", FieldDefault::None),
        FieldSpec::new(
            "reason",
            FieldDefault::NativeMember(RecordId::PlanReason, "NO_CHANGE"),
        ),
    ],
};
const R_AUDIT_PLAN_ENTRY: RecordSpec = RecordSpec {
    name: "AuditPlanEntry",
    doc: "Per-audit execution plan entry with resolved SQL and scheduling metadata.",
    fields: &[
        FieldSpec::required("key"),
        FieldSpec::required("name"),
        FieldSpec::required("definition_name"),
        FieldSpec::required("resolved_sql"),
        FieldSpec::required("unresolved_sql"),
        FieldSpec::required("attachment_kind"),
        FieldSpec::required("severity"),
        FieldSpec::required("requested_run_scope"),
        FieldSpec::required("effective_run_scope"),
        FieldSpec::new(
            "evaluation_mode",
            FieldDefault::Member(
                "sqlbuild.compiler.auditing.types",
                "AuditEvaluationMode",
                "VIOLATIONS",
            ),
        ),
        FieldSpec::new("value_column", FieldDefault::None),
        FieldSpec::new("sample_count_column", FieldDefault::None),
        FieldSpec::new("sample_unit", FieldDefault::None),
        FieldSpec::new("thresholds", FieldDefault::None),
        FieldSpec::new("minimum_samples", FieldDefault::None),
        FieldSpec::new("evidence_resolved_sql", FieldDefault::None),
        FieldSpec::new("evidence_unresolved_sql", FieldDefault::None),
        FieldSpec::new("evidence_limit", FieldDefault::None),
        FieldSpec::new("scope_deps", FieldDefault::EmptyTuple),
        FieldSpec::new("attached_target_kind", FieldDefault::None),
        FieldSpec::new("attached_target_name", FieldDefault::None),
        FieldSpec::new("attached_column_name", FieldDefault::None),
        FieldSpec::new("always_run", FieldDefault::False),
        FieldSpec::new("description", FieldDefault::None),
        FieldSpec::new("reads_outside_target_lineage", FieldDefault::False),
        FieldSpec::new("meta", FieldDefault::None),
    ],
};
const R_CHAIN_STEP: RecordSpec = RecordSpec {
    name: "ChainStep",
    doc: "One step in a chained test execution sequence.",
    fields: &[
        FieldSpec::required("model_name"),
        FieldSpec::required("resolved_sql"),
        FieldSpec::new("expected_cte_sql", FieldDefault::None),
        FieldSpec::new("lifted_ctes", FieldDefault::EmptyTuple),
        FieldSpec::new("comparison_body_sql", FieldDefault::None),
        FieldSpec::new("expected_columns", FieldDefault::None),
        FieldSpec::new("expected_lifted_ctes", FieldDefault::EmptyTuple),
    ],
};
const R_SQL_TEST_PLAN_ENTRY: RecordSpec = RecordSpec {
    name: "SqlTestPlanEntry",
    doc: "Per-test execution plan entry with chained resolution.",
    fields: &[
        FieldSpec::required("key"),
        FieldSpec::required("name"),
        FieldSpec::new("source_path", FieldDefault::None),
        FieldSpec::new("block_index", FieldDefault::None),
        FieldSpec::new("parent_name", FieldDefault::None),
        FieldSpec::new("case_name", FieldDefault::None),
        FieldSpec::new("case_index", FieldDefault::None),
        FieldSpec::new("case_fingerprint", FieldDefault::None),
        FieldSpec::new("parameter_schema", FieldDefault::EmptyTuple),
        FieldSpec::new("parameter_values", FieldDefault::EmptyTuple),
        FieldSpec::new("mock_ref_names", FieldDefault::EmptyTuple),
        FieldSpec::new("mock_source_names", FieldDefault::EmptyTuple),
        FieldSpec::new("mock_seed_names", FieldDefault::EmptyTuple),
        FieldSpec::new("mock_dbt_ref_names", FieldDefault::EmptyTuple),
        FieldSpec::new("mock_table_function_names", FieldDefault::EmptyTuple),
        FieldSpec::new("chain", FieldDefault::EmptyTuple),
        FieldSpec::new("assertions", FieldDefault::EmptyTuple),
        FieldSpec::new("scope_deps", FieldDefault::EmptyTuple),
        FieldSpec::new("function_deps", FieldDefault::EmptyTuple),
        FieldSpec::new("sql_analysis_enabled", FieldDefault::True),
        FieldSpec::new("real_model_count", FieldDefault::None),
        FieldSpec::new("timeout", FieldDefault::None),
        FieldSpec::new("timeout_seconds", FieldDefault::None),
    ],
};
const R_SCENARIO_RELATION_MAP: RecordSpec = RecordSpec {
    name: "ScenarioRelationMap",
    doc: "Resolved scenario hash prefix and scenario-owned relation names.",
    fields: &[
        FieldSpec::required("scenario_name"),
        FieldSpec::required("hash_prefix"),
        FieldSpec::new("artifacts", FieldDefault::EmptyTuple),
    ],
};
const R_SCENARIO_RELATION_PLAN: RecordSpec = RecordSpec {
    name: "ScenarioRelationPlan",
    doc: "Resolved scenario relation locations for model/source/seed ref resolution.",
    fields: &[
        FieldSpec::required("scenario_name"),
        FieldSpec::required("relation_map"),
        FieldSpec::new("model_locations", FieldDefault::EmptyDict),
        FieldSpec::new("seed_locations", FieldDefault::EmptyDict),
        FieldSpec::new("project_source_map", FieldDefault::EmptyDict),
        FieldSpec::new("source_map", FieldDefault::EmptyDict),
        FieldSpec::new("source_fixture_locations", FieldDefault::EmptyDict),
        FieldSpec::new("ref_fixture_locations", FieldDefault::EmptyDict),
        FieldSpec::new("seed_fixture_locations", FieldDefault::EmptyDict),
        FieldSpec::new("dbt_ref_fixture_locations", FieldDefault::EmptyDict),
    ],
};
const R_SCENARIO_FIXTURE_PLAN: RecordSpec = RecordSpec {
    name: "ScenarioFixturePlan",
    doc: "Self-contained fixture SQL planned for scenario materialization.",
    fields: &[
        FieldSpec::required("kind"),
        FieldSpec::required("logical_name"),
        FieldSpec::required("destination"),
        FieldSpec::required("sql"),
        FieldSpec::new("fingerprint_sql", FieldDefault::Text("")),
    ],
};
const R_SCENARIO_EXPECTED_EXPECTATION_PLAN: RecordSpec = RecordSpec {
    name: "ScenarioExpectedExpectationPlan",
    doc: "Expected-output comparison inputs for one scenario target model.",
    fields: &[
        FieldSpec::required("model_name"),
        FieldSpec::required("actual_destination"),
        FieldSpec::required("expected_sql"),
        FieldSpec::new("compared_columns", FieldDefault::None),
    ],
};
const R_SCENARIO_ASSERTION_EXPECTATION_PLAN: RecordSpec = RecordSpec {
    name: "ScenarioAssertionExpectationPlan",
    doc: "Zero-row assertion SQL for one scenario assertion CTE.",
    fields: &[FieldSpec::required("name"), FieldSpec::required("sql")],
};
const R_SCENARIO_EXECUTION_PLAN: RecordSpec = RecordSpec {
    name: "ScenarioExecutionPlan",
    doc: "Dry-run execution plan for a SQL scenario graph slice.",
    fields: &[
        FieldSpec::required("key"),
        FieldSpec::required("name"),
        FieldSpec::required("graph_plan"),
        FieldSpec::required("relation_plan"),
        FieldSpec::new("fixture_plans", FieldDefault::EmptyTuple),
        FieldSpec::new("seed_entries", FieldDefault::EmptyTuple),
        FieldSpec::new("function_entries", FieldDefault::EmptyTuple),
        FieldSpec::new("model_entries", FieldDefault::EmptyTuple),
        FieldSpec::new("hook_functions", FieldDefault::EmptyTuple),
        FieldSpec::new("expected_expectations", FieldDefault::EmptyTuple),
        FieldSpec::new("assertion_expectations", FieldDefault::EmptyTuple),
    ],
};
const R_PLAN_OUTPUT: RecordSpec = RecordSpec {
    name: "PlanOutput",
    doc: "Complete execution plan produced by the planner.",
    fields: &[
        FieldSpec::new("execution_order", FieldDefault::EmptyTuple),
        FieldSpec::new("model_entries", FieldDefault::EmptyTuple),
        FieldSpec::new("seed_entries", FieldDefault::EmptyTuple),
        FieldSpec::new("source_load_entries", FieldDefault::EmptyTuple),
        FieldSpec::new("function_entries", FieldDefault::EmptyTuple),
        FieldSpec::new("audit_entries", FieldDefault::EmptyTuple),
        FieldSpec::new("test_entries", FieldDefault::EmptyTuple),
        FieldSpec::new("available_test_case_names", FieldDefault::EmptyTuple),
        FieldSpec::new("selected_keys", FieldDefault::EmptyFrozenset),
        FieldSpec::new("warnings", FieldDefault::EmptyTuple),
        FieldSpec::new("retention_entries", FieldDefault::EmptyTuple),
        FieldSpec::new("table_type_entries", FieldDefault::EmptyTuple),
        FieldSpec::new("migration_entries", FieldDefault::EmptyTuple),
        FieldSpec::new("column_migration_entries", FieldDefault::EmptyTuple),
        FieldSpec::new("old_name_view_entries", FieldDefault::EmptyTuple),
        FieldSpec::new("old_name_views", FieldDefault::EmptyTuple),
        FieldSpec::new("upstream_deps", FieldDefault::EmptyDict),
        FieldSpec::new("downstream_deps", FieldDefault::EmptyDict),
        FieldSpec::new("model_locations", FieldDefault::EmptyDict),
        FieldSpec::new("seed_locations", FieldDefault::EmptyDict),
        FieldSpec::new("function_locations", FieldDefault::EmptyDict),
        FieldSpec::new("source_map", FieldDefault::EmptyDict),
        FieldSpec::new("source_read_map", FieldDefault::EmptyDict),
        FieldSpec::new("python_source_read_map", FieldDefault::EmptyDict),
        FieldSpec::new("hook_functions", FieldDefault::EmptyTuple),
        FieldSpec::new("enforce_explicit_references", FieldDefault::True),
        FieldSpec::new("provider_usages", FieldDefault::EmptyTuple),
        FieldSpec::new("source_freshness", FieldDefault::None),
        FieldSpec::new("python_identity_fingerprints", FieldDefault::EmptyDict),
        FieldSpec::new("recorded_fingerprints", FieldDefault::None),
        FieldSpec::new("metadata", FieldDefault::EmptyDict),
        FieldSpec::new("cursor_overrides", FieldDefault::None),
        FieldSpec::new("existing_model_names", FieldDefault::EmptyFrozenset),
        FieldSpec::new("end_audit_source_keys", FieldDefault::EmptyFrozenset),
    ],
};

/// The planner, migration, cursor and fingerprint value records.
pub(crate) const PLANNER_RECORDS: &[RecordSpec] = &[
    R_PARSED_SELECTOR,
    R_PATH_SELECTOR,
    R_SQL_TEST_SELECTION,
    R_PLANNER_SCOPE,
    R_SQL_TEST_ASSERTION_STEP,
    R_PLANNER_SELECTION,
    R_PLANNER_OVERRIDES,
    R_DEFERRAL_INPUTS,
    R_PLANNER_POLICIES,
    R_DIRECT_SOURCE_FRESHNESS_PLANNING_RESULT,
    R_FINGERPRINT,
    R_FINGERPRINT_SET,
    R_SOURCE_FRESHNESS_RECORD,
    R_TIMESTAMP_VALUE,
    R_DATE_VALUE,
    R_INTEGER_VALUE,
    R_ALIGNED_INTERVAL,
    R_DURATION,
    R_MIGRATION_RELATION,
    R_MIGRATION_EVENT,
    R_COLUMN_MIGRATION_EVENT,
    R_OLD_NAME_VIEW_EVENT,
    R_OLD_NAME_VIEW_HISTORY,
    R_CURSOR_OVERRIDES,
    R_EMPTY_CURSOR_INPUT_DECISION,
    R_MODEL_CURSOR_SNAPSHOT,
    R_CURSOR_BOUNDS,
    R_CURSOR_INPUT_EVIDENCE,
    R_FUTURE_CURSOR_SAFETY_EVIDENCE,
    R_MAXIMUM_START_SAFETY_EVIDENCE,
    R_CURSOR_INPUT_RELATION,
    R_MAXIMUM_START_POLICY_INPUTS,
    R_SCHEMA_FINDING,
    R_RETENTION_PLAN_ENTRY,
    R_TABLE_TYPE_PLAN_ENTRY,
    R_MODEL_MIGRATION_PLAN_ENTRY,
    R_OLD_NAME_VIEW_PLAN_ENTRY,
    R_OLD_NAME_VIEW,
    R_COLUMN_MIGRATION_PLAN_ENTRY,
    R_MODEL_PLAN_ENTRY,
    R_CLONE_SOURCE_PLAN_ENTRY,
    R_SEED_PLAN_ENTRY,
    R_SOURCE_LOAD_PLAN_ENTRY,
    R_FUNCTION_PLAN_ENTRY,
    R_AUDIT_PLAN_ENTRY,
    R_CHAIN_STEP,
    R_SQL_TEST_PLAN_ENTRY,
    R_SCENARIO_RELATION_MAP,
    R_SCENARIO_RELATION_PLAN,
    R_SCENARIO_FIXTURE_PLAN,
    R_SCENARIO_EXPECTED_EXPECTATION_PLAN,
    R_SCENARIO_ASSERTION_EXPECTATION_PLAN,
    R_SCENARIO_EXECUTION_PLAN,
    R_PLAN_OUTPUT,
];

const R_NATIVE_SQL_TEST_ARTIFACT: RecordSpec = RecordSpec {
    name: "NativeSqlTestArtifact",
    doc: "One native-planned SQL-test artifact and its distinct planning error messages.",
    fields: &[
        FieldSpec::required("sql"),
        FieldSpec::required("model_names"),
        FieldSpec::new("error_messages", FieldDefault::EmptyTuple),
    ],
};
const R_RELATION_FIXTURE_DIAGNOSTIC: RecordSpec = RecordSpec {
    name: "RelationFixtureDiagnostic",
    doc: "One statically provable invalid fixture condition.",
    fields: &[FieldSpec::required("key"), FieldSpec::required("message")],
};
const R_RELATION_FIXTURE_COMPLETION: RecordSpec = RecordSpec {
    name: "RelationFixtureCompletion",
    doc: "Completed fixture SQL plus metadata reused by SQL-test validation.",
    fields: &[
        FieldSpec::required("fixture_sql_by_key"),
        FieldSpec::required("inferred_by_fixture"),
        FieldSpec::required("expected_types"),
        FieldSpec::required("diagnostics"),
    ],
};
const R_SELECTOR_EXPANSION: RecordSpec = RecordSpec {
    name: "SelectorExpansion",
    doc: "One selector split into graph expansion flags and core text.",
    fields: &[
        FieldSpec::required("core"),
        FieldSpec::new("upstream", FieldDefault::False),
        FieldSpec::new("downstream", FieldDefault::False),
    ],
};
const R_PARSED_SCENARIO_ARTIFACT_NAME: RecordSpec = RecordSpec {
    name: "ParsedScenarioArtifactName",
    doc: "Parsed physical name for one planner-owned scenario artifact.",
    fields: &[
        FieldSpec::required("hash_prefix"),
        FieldSpec::required("kind"),
        FieldSpec::required("logical_name"),
    ],
};
const R_MISSING_UPSTREAM: RecordSpec = RecordSpec {
    name: "MissingUpstream",
    doc: "One upstream dependency missing from both scope and warehouse.",
    fields: &[
        FieldSpec::required("key"),
        FieldSpec::required("required_by"),
    ],
};
const R_WAREHOUSE_FINGERPRINTS: RecordSpec = RecordSpec {
    name: "WarehouseFingerprints",
    doc: "Latest direct fingerprints grouped by node type.",
    fields: &[
        FieldSpec::new("models", FieldDefault::EmptyDict),
        FieldSpec::new("functions", FieldDefault::EmptyDict),
        FieldSpec::new("seeds", FieldDefault::EmptyDict),
        FieldSpec::new("python_nodes", FieldDefault::EmptyDict),
        FieldSpec::uncompared("unfiltered_schemas", FieldDefault::EmptyDict),
        FieldSpec::uncompared("unfiltered_database", FieldDefault::None),
    ],
};
const R_CURSOR_SNAPSHOT_SCOPE: RecordSpec = RecordSpec {
    name: "CursorSnapshotScope",
    doc: "Execution selection used for cursor validation and runtime producer resolution.",
    fields: &[
        FieldSpec::required("model_keys"),
        FieldSpec::required("runtime_producer_keys"),
        FieldSpec::new("invocation_time", FieldDefault::None),
        FieldSpec::new("start_cursor_config", FieldDefault::None),
        FieldSpec::new("cursor_overrides", FieldDefault::None),
    ],
};
const R_REFERENCE_RENAME: RecordSpec = RecordSpec {
    name: "ReferenceRename",
    doc: "One model rename that downstream references may have been rewritten for.",
    fields: &[
        FieldSpec::required("new_name"),
        FieldSpec::required("origin_name"),
        FieldSpec::new("recorded_at", FieldDefault::None),
    ],
};
const R_WAREHOUSE_SNAPSHOT: RecordSpec = RecordSpec {
    name: "WarehouseSnapshot",
    doc: "Frozen point-in-time picture of warehouse state for planning.",
    fields: &[
        FieldSpec::new("existing_relations", FieldDefault::EmptyDict),
        FieldSpec::new("existing_columns", FieldDefault::EmptyDict),
        FieldSpec::new(
            "fingerprints",
            FieldDefault::NativeCall(RecordId::WarehouseFingerprints),
        ),
        FieldSpec::new("cursor_snapshots", FieldDefault::EmptyDict),
        FieldSpec::new(
            "source_freshness_state_schemas",
            FieldDefault::EmptyFrozenset,
        ),
        FieldSpec::new("column_dialect", FieldDefault::None),
        FieldSpec::new("renamed_models", FieldDefault::EmptyFrozenset),
        FieldSpec::new("reference_renames", FieldDefault::EmptyTuple),
        FieldSpec::new("migration_state_schemas", FieldDefault::EmptyFrozenset),
        FieldSpec::new("old_name_view_state_schemas", FieldDefault::EmptyFrozenset),
        FieldSpec::new("listed_state_schemas", FieldDefault::EmptyFrozenset),
    ],
};
const R_SELECTION_STALENESS_NODE_KEY: RecordSpec = RecordSpec {
    name: "SelectionStalenessNodeKey",
    doc: "Neutral node identity for shared selection-aware staleness classification.",
    fields: &[
        FieldSpec::required("resource_type"),
        FieldSpec::required("name"),
    ],
};
const R_SELECTION_STALENESS_GRAPH: RecordSpec = RecordSpec {
    name: "SelectionStalenessGraph",
    doc: "Neutral graph inputs for selected-model stale warning classification.",
    fields: &[
        FieldSpec::required("upstream_deps"),
        FieldSpec::required("selected_model_names"),
        FieldSpec::required("run_model_names"),
        FieldSpec::required("run_seed_names"),
        FieldSpec::required("run_source_names"),
        FieldSpec::required("changed_model_names"),
        FieldSpec::required("changed_seed_names"),
        FieldSpec::required("changed_source_names"),
    ],
};
const R_SELECTION_STALENESS_WARNING: RecordSpec = RecordSpec {
    name: "SelectionStalenessWarning",
    doc: "Neutral stale warning classification for one selected model.",
    fields: &[
        FieldSpec::required("model_name"),
        FieldSpec::required("trigger_names"),
    ],
};
const R_BACKFILL_RESULT: RecordSpec = RecordSpec {
    name: "BackfillResult",
    doc: "Resolved backfill action from a change detection policy.",
    fields: &[
        FieldSpec::required("action"),
        FieldSpec::new("duration", FieldDefault::None),
    ],
};
const R_CHANGE_DETECTION_RESULT: RecordSpec = RecordSpec {
    name: "ChangeDetectionResult",
    doc: "Per-model output from change detection and policy resolution.",
    fields: &[
        FieldSpec::required("model_name"),
        FieldSpec::required("change_kind"),
        FieldSpec::new("query_changed", FieldDefault::False),
        FieldSpec::new("config_changed", FieldDefault::False),
        FieldSpec::new("fingerprint_metadata_json", FieldDefault::None),
        FieldSpec::new("previous_metadata_json", FieldDefault::None),
        FieldSpec::new("fingerprint_version_hash", FieldDefault::None),
        FieldSpec::new("previous_version_hash", FieldDefault::None),
        FieldSpec::new("fingerprint_definition_hash", FieldDefault::None),
        FieldSpec::new("schema_findings", FieldDefault::EmptyTuple),
        FieldSpec::new("backfill", FieldDefault::Factory(forward_only_backfill)),
        FieldSpec::new("recorded_build_relation_missing", FieldDefault::False),
        FieldSpec::new("changed_functions", FieldDefault::EmptyTuple),
    ],
};
const R_UNIT_TEST_SELECTOR_SPLIT: RecordSpec = RecordSpec {
    name: "UnitTestSelectorSplit",
    doc: "Resource selectors left after unit-test selectors moved into a test selection.",
    fields: &[
        FieldSpec::required("select"),
        FieldSpec::required("exclude"),
        FieldSpec::required("sql_test_selection"),
    ],
};
const R_CURSOR_OVERRIDE_PAIR: RecordSpec = RecordSpec {
    name: "CursorOverridePair",
    doc: "Resolved per-model cursor start/end overrides for plan entry construction.",
    fields: &[
        FieldSpec::new("start_cursor_override", FieldDefault::None),
        FieldSpec::new("end_cursor_override", FieldDefault::None),
    ],
};
const R_FUNCTION_CHANGE_RESULT: RecordSpec = RecordSpec {
    name: "FunctionChangeResult",
    doc: "Per-function output from change detection.",
    fields: &[
        FieldSpec::required("fingerprint_sql"),
        FieldSpec::new(
            "reason",
            FieldDefault::NativeMember(RecordId::PlanReason, "NO_CHANGE"),
        ),
    ],
};
const R_PLANNER_CHANGE_RESULTS: RecordSpec = RecordSpec {
    name: "PlannerChangeResults",
    doc: "Change detection output for selected planner resources.",
    fields: &[
        FieldSpec::required("models"),
        FieldSpec::required("functions"),
    ],
};
const R_RESOLVED_MODEL_ACTION: RecordSpec = RecordSpec {
    name: "ResolvedModelAction",
    doc: "Effective model change and backfill decided from the model's own change.",
    fields: &[
        FieldSpec::required("change"),
        FieldSpec::required("backfill"),
    ],
};
const R_PLANNER_RESOLVED_ACTIONS: RecordSpec = RecordSpec {
    name: "PlannerResolvedActions",
    doc: "Per-model planning decisions keyed by model name.",
    fields: &[FieldSpec::required("models")],
};
const R_DIRECT_MODEL_VERSION_IDENTITIES: RecordSpec = RecordSpec {
    name: "DirectModelVersionIdentities",
    doc: "Current direct model version identity values by model name.",
    fields: &[
        FieldSpec::required("function_local_hashes"),
        FieldSpec::required("seed_version_hashes"),
        FieldSpec::required("seed_metadata_jsons"),
        FieldSpec::required("model_metadata_jsons"),
        FieldSpec::required("model_local_hashes"),
        FieldSpec::required("model_version_hashes"),
    ],
};
const R_SCHEMA_ACTION: RecordSpec = RecordSpec {
    name: "SchemaAction",
    doc: "One concrete schema change action to apply to a target table.",
    fields: &[
        FieldSpec::required("kind"),
        FieldSpec::required("column_name"),
        FieldSpec::new("column_type", FieldDefault::None),
    ],
};
const R_PLAN_WARNING: RecordSpec = RecordSpec {
    name: "PlanWarning",
    doc: "One warning or error produced during plan resolution.",
    fields: &[
        FieldSpec::required("model_name"),
        FieldSpec::required("severity"),
        FieldSpec::required("message"),
        FieldSpec::new("code", FieldDefault::None),
    ],
};
const R_COLUMN_RENAME_HINT: RecordSpec = RecordSpec {
    name: "ColumnRenameHint",
    doc: "A removed column that an added column may have been renamed from.",
    fields: &[
        FieldSpec::required("model_name"),
        FieldSpec::required("added_column"),
        FieldSpec::required("candidate_columns"),
        FieldSpec::new("identical", FieldDefault::False),
    ],
};
const R_QUERY_PROJECTION: RecordSpec = RecordSpec {
    name: "QueryProjection",
    doc: "One named top-level output column of a query and its canonical expression.",
    fields: &[
        FieldSpec::required("name"),
        FieldSpec::required("expression"),
        FieldSpec::required("references"),
        FieldSpec::new("passthrough", FieldDefault::False),
    ],
};
const R_QUERY_SHAPE: RecordSpec = RecordSpec {
    name: "QueryShape",
    doc: "Canonical top-level projections and the rest of one query, ignoring formatting.",
    fields: &[
        FieldSpec::required("projections"),
        FieldSpec::required("body"),
        FieldSpec::new("alias_clauses", FieldDefault::Text("{}")),
        FieldSpec::new("positional", FieldDefault::False),
        FieldSpec::new("input_relations", FieldDefault::None),
        FieldSpec::new("local_input_columns", FieldDefault::EmptyFrozenset),
        FieldSpec::new("clause_references", FieldDefault::EmptyFrozenset),
    ],
};
const R_PLAN_OUTPUT_EXTRAS: RecordSpec = RecordSpec {
    name: "PlanOutputExtras",
    doc: "Optional supplemental seed fingerprints and precomputed SQL tests for plan assembly.",
    fields: &[
        FieldSpec::new("seed_version_hashes", FieldDefault::None),
        FieldSpec::new("seed_metadata_jsons", FieldDefault::None),
        FieldSpec::new("seed_plan_reasons", FieldDefault::None),
        FieldSpec::new("planned_sql_tests", FieldDefault::None),
    ],
};
const R_PLANNED_SQL_TESTS: RecordSpec = RecordSpec {
    name: "PlannedSqlTests",
    doc: "SQL test entries planned for one selection; reusable only for the same selection.",
    fields: &[
        FieldSpec::required("selected_keys"),
        FieldSpec::new(
            "sql_test_selection",
            FieldDefault::NativeCall(RecordId::SqlTestSelection),
        ),
        FieldSpec::new("entries", FieldDefault::EmptyTuple),
        FieldSpec::new("warnings", FieldDefault::EmptyTuple),
    ],
};
const R_NATIVE_SQL_TEST_PLAN: RecordSpec = RecordSpec {
    name: "NativeSqlTestPlan",
    doc: "Native chain and assertion steps for one SQL test, with SQL when rendered.",
    fields: &[
        FieldSpec::required("chain"),
        FieldSpec::required("assertions"),
        FieldSpec::required("model_names"),
        FieldSpec::required("warnings"),
        FieldSpec::required("error_messages"),
        FieldSpec::new("sql", FieldDefault::None),
    ],
};
const R_SQL_TEST_PLAN_RESULT: RecordSpec = RecordSpec {
    name: "SqlTestPlanResult",
    doc: "One SQL test's plan entry and warnings, or the fixture diagnostics preventing it.",
    fields: &[
        FieldSpec::required("entry"),
        FieldSpec::new("warnings", FieldDefault::EmptyTuple),
        FieldSpec::new("fixture_diagnostics", FieldDefault::EmptyTuple),
    ],
};
const R_SCENARIO_GRAPH_PLAN: RecordSpec = RecordSpec {
    name: "ScenarioGraphPlan",
    doc: "Inferred graph slice and fixture boundaries for one SQL scenario.",
    fields: &[
        FieldSpec::required("key"),
        FieldSpec::required("name"),
        FieldSpec::new("target_model_names", FieldDefault::EmptyTuple),
        FieldSpec::new("assertion_target_model_names", FieldDefault::EmptyTuple),
        FieldSpec::new("model_names", FieldDefault::EmptyTuple),
        FieldSpec::new("source_fixture_names", FieldDefault::EmptyTuple),
        FieldSpec::new("ref_fixture_names", FieldDefault::EmptyTuple),
        FieldSpec::new("seed_names", FieldDefault::EmptyTuple),
        FieldSpec::new("seed_fixture_names", FieldDefault::EmptyTuple),
        FieldSpec::new("dbt_ref_fixture_names", FieldDefault::EmptyTuple),
        FieldSpec::new("function_deps", FieldDefault::EmptyTuple),
    ],
};
const R_SCENARIO_ARTIFACT_IDENTITY: RecordSpec = RecordSpec {
    name: "ScenarioArtifactIdentity",
    doc: "Logical identity for one scenario-owned physical artifact.",
    fields: &[
        FieldSpec::required("kind"),
        FieldSpec::required("logical_name"),
    ],
};
const R_SCENARIO_ARTIFACT_NAME: RecordSpec = RecordSpec {
    name: "ScenarioArtifactName",
    doc: "Resolved physical relation name for one scenario artifact.",
    fields: &[
        FieldSpec::required("identity"),
        FieldSpec::required("physical_name"),
    ],
};
const R_PLAN_PROVIDER_USAGE: RecordSpec = RecordSpec {
    name: "PlanProviderUsage",
    doc: "Provider usage metadata scoped to planned work.",
    fields: &[
        FieldSpec::required("provider_name"),
        FieldSpec::required("consumer_kind"),
        FieldSpec::required("consumer_name"),
        FieldSpec::required("parameter_name"),
        FieldSpec::new("annotation_class_name", FieldDefault::None),
        FieldSpec::new("annotation_module", FieldDefault::None),
    ],
};

/// Planner value records, in registration order.
pub(crate) const PLANNER_VALUE_RECORDS: &[RecordSpec] = &[
    R_NATIVE_SQL_TEST_ARTIFACT,
    R_RELATION_FIXTURE_DIAGNOSTIC,
    R_RELATION_FIXTURE_COMPLETION,
    R_SELECTOR_EXPANSION,
    R_PARSED_SCENARIO_ARTIFACT_NAME,
    R_MISSING_UPSTREAM,
    R_WAREHOUSE_FINGERPRINTS,
    R_CURSOR_SNAPSHOT_SCOPE,
    R_REFERENCE_RENAME,
    R_WAREHOUSE_SNAPSHOT,
    R_SELECTION_STALENESS_NODE_KEY,
    R_SELECTION_STALENESS_GRAPH,
    R_SELECTION_STALENESS_WARNING,
    R_BACKFILL_RESULT,
    R_CHANGE_DETECTION_RESULT,
    R_UNIT_TEST_SELECTOR_SPLIT,
    R_CURSOR_OVERRIDE_PAIR,
    R_FUNCTION_CHANGE_RESULT,
    R_PLANNER_CHANGE_RESULTS,
    R_RESOLVED_MODEL_ACTION,
    R_PLANNER_RESOLVED_ACTIONS,
    R_DIRECT_MODEL_VERSION_IDENTITIES,
    R_SCHEMA_ACTION,
    R_PLAN_WARNING,
    R_COLUMN_RENAME_HINT,
    R_QUERY_PROJECTION,
    R_QUERY_SHAPE,
    R_PLAN_OUTPUT_EXTRAS,
    R_PLANNED_SQL_TESTS,
    R_NATIVE_SQL_TEST_PLAN,
    R_SQL_TEST_PLAN_RESULT,
    R_SCENARIO_GRAPH_PLAN,
    R_SCENARIO_ARTIFACT_IDENTITY,
    R_SCENARIO_ARTIFACT_NAME,
    R_PLAN_PROVIDER_USAGE,
];

/// Planner string enums, in declaration order.
pub(crate) const PLANNER_ENUMS: &[EnumSpec] = &[
    EnumSpec {
        name: "GraphResourceKind",
        doc: "",
        members: &[
            ("MODEL", "model"),
            ("SEED", "seed"),
            ("SOURCE", "source"),
            ("FUNCTION", "function"),
        ],
    },
    EnumSpec {
        name: "ChangeKind",
        doc: "",
        members: &[
            ("FIRST_RUN", "first_run"),
            ("RENAMED", "renamed"),
            ("QUERY_CHANGED", "query_changed"),
            ("FUNCTION_CHANGED", "function_changed"),
            ("CONFIG_CHANGED", "config_changed"),
            ("SCHEMA_CHANGED", "schema_changed"),
            ("NO_CHANGE", "no_change"),
        ],
    },
    EnumSpec {
        name: "SchemaChangeKind",
        doc: "",
        members: &[
            ("COLUMN_ADDED", "column_added"),
            ("COLUMN_REMOVED", "column_removed"),
            ("COLUMN_TYPE_CHANGED", "column_type_changed"),
        ],
    },
    EnumSpec {
        name: "SchemaColumnSource",
        doc: "",
        members: &[("YML", "yml"), ("SQL_ANALYSIS", "sql_analysis")],
    },
    EnumSpec {
        name: "CursorWatermarkMode",
        doc: "",
        members: &[("ALL", "all"), ("ANY", "any")],
    },
    EnumSpec {
        name: "SchemaActionKind",
        doc: "",
        members: &[
            ("ADD_COLUMN", "add_column"),
            ("DROP_COLUMN", "drop_column"),
            ("ALTER_COLUMN_TYPE", "alter_column_type"),
        ],
    },
    EnumSpec {
        name: "WarningSeverity",
        doc: "",
        members: &[("ERROR", "error"), ("WARNING", "warning"), ("INFO", "info")],
    },
];
