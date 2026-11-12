//! Layouts of the project configuration and contract value records hosted natively.

use crate::bindings::_helpers::records::ids::RecordId;
use crate::bindings::_helpers::records::models::{FieldDefault, FieldSpec, RecordSpec};

const R_CLONE_POLICY: RecordSpec = RecordSpec {
    name: "ClonePolicy",
    doc: "Environment clone policy.",
    fields: &[
        FieldSpec::new("allow_as_clone_origin", FieldDefault::False),
        FieldSpec::new("allow_as_clone_destination", FieldDefault::False),
    ],
};
const R_LOCAL_CLONE_POLICY: RecordSpec = RecordSpec {
    name: "LocalClonePolicy",
    doc: "Local environment clone policy overrides.",
    fields: &[
        FieldSpec::new("allow_as_clone_origin", FieldDefault::None),
        FieldSpec::new("allow_as_clone_destination", FieldDefault::None),
    ],
};
const R_AUTHORED_TIME_TRAVEL_RETENTION: RecordSpec = RecordSpec {
    name: "AuthoredTimeTravelRetention",
    doc: "One authored time-travel retention policy.",
    fields: &[
        FieldSpec::new("desired_days", FieldDefault::None),
        FieldSpec::new("unmanaged", FieldDefault::False),
    ],
};
const R_MATERIALIZATION_RETENTION_DEFAULTS: RecordSpec = RecordSpec {
    name: "MaterializationRetentionDefaults",
    doc: "Retention defaults for one built-in materialization.",
    fields: &[
        FieldSpec::new("time_travel_retention", FieldDefault::None),
        FieldSpec::new("table_type", FieldDefault::None),
    ],
};
const R_MATERIALIZATION_DEFAULTS_CONFIG: RecordSpec = RecordSpec {
    name: "MaterializationDefaultsConfig",
    doc: "Project retention defaults by materialization type.",
    fields: &[
        FieldSpec::new(
            "table",
            FieldDefault::NativeCall(RecordId::MaterializationRetentionDefaults),
        ),
        FieldSpec::new(
            "incremental",
            FieldDefault::NativeCall(RecordId::MaterializationRetentionDefaults),
        ),
        FieldSpec::new(
            "snapshot",
            FieldDefault::NativeCall(RecordId::MaterializationRetentionDefaults),
        ),
    ],
};
const R_RESOLVED_TIME_TRAVEL_RETENTION: RecordSpec = RecordSpec {
    name: "ResolvedTimeTravelRetention",
    doc: "Effective retention policy attached to a compiled model.",
    fields: &[
        FieldSpec::new("desired_days", FieldDefault::None),
        FieldSpec::new("unmanaged", FieldDefault::True),
        FieldSpec::new("source", FieldDefault::None),
    ],
};
const R_RESOLVED_TABLE_TYPE: RecordSpec = RecordSpec {
    name: "ResolvedTableType",
    doc: "Effective Snowflake table type attached to a compiled model.",
    fields: &[
        FieldSpec::new(
            "value",
            FieldDefault::NativeMember(RecordId::TableType, "TRANSIENT"),
        ),
        FieldSpec::new(
            "source",
            FieldDefault::NativeMember(RecordId::TableTypeSource, "DEFAULT"),
        ),
        FieldSpec::new("declared", FieldDefault::False),
    ],
};
const R_EXECUTION_LIMITS_CONFIG: RecordSpec = RecordSpec {
    name: "ExecutionLimitsConfig",
    doc: "Optional target-scoped build execution limits.",
    fields: &[
        FieldSpec::new("max_models", FieldDefault::None),
        FieldSpec::new("max_duration", FieldDefault::None),
        FieldSpec::new("max_duration_seconds", FieldDefault::None),
        FieldSpec::new("remediation", FieldDefault::None),
        FieldSpec::new("test_timeout", FieldDefault::None),
        FieldSpec::new("test_timeout_seconds", FieldDefault::None),
        FieldSpec::new("scenario_timeout", FieldDefault::None),
        FieldSpec::new("scenario_timeout_seconds", FieldDefault::None),
    ],
};
const R_TARGET_DIFF_CONFIG: RecordSpec = RecordSpec {
    name: "TargetDiffConfig",
    doc: "Optional target-scoped data-diff limits.",
    fields: &[FieldSpec::new("max_full_rows", FieldDefault::None)],
};
const R_TARGET_WAREHOUSES_CONFIG: RecordSpec = RecordSpec {
    name: "TargetWarehousesConfig",
    doc: "Optional per-command-group default warehouses for one target.",
    fields: &[
        FieldSpec::new("build", FieldDefault::None),
        FieldSpec::new("query", FieldDefault::None),
    ],
};
const R_TARGET_CONFIG: RecordSpec = RecordSpec {
    name: "TargetConfig",
    doc: "One named target configuration.",
    fields: &[
        FieldSpec::new("connection", FieldDefault::EmptyDict),
        FieldSpec::new("connection_name", FieldDefault::None),
        FieldSpec::new("vars", FieldDefault::EmptyDict),
        FieldSpec::new("database", FieldDefault::None),
        FieldSpec::new("schema", FieldDefault::None),
        FieldSpec::new("loader_schema", FieldDefault::None),
        FieldSpec::new("defer_sources_to", FieldDefault::None),
        FieldSpec::new("defer_clone_from", FieldDefault::None),
        FieldSpec::new("clone", FieldDefault::NativeCall(RecordId::ClonePolicy)),
        FieldSpec::new("compile_cache", FieldDefault::None),
        FieldSpec::new("time_travel_retention", FieldDefault::None),
        FieldSpec::new(
            "time_travel_retention_by_materialization",
            FieldDefault::EmptyDict,
        ),
        FieldSpec::new("owns_time_travel_retention_namespace", FieldDefault::False),
        FieldSpec::new("default_table_type", FieldDefault::None),
        FieldSpec::new(
            "table_type_downgrade",
            FieldDefault::NativeMember(RecordId::TableTypeDowngradePolicy, "REQUIRE_CONFIRMATION"),
        ),
        FieldSpec::new(
            "time_travel_retention_decrease",
            FieldDefault::NativeMember(RecordId::RetentionDecreasePolicy, "DENY"),
        ),
        FieldSpec::new(
            "missing_migration_origin",
            FieldDefault::NativeMember(RecordId::MissingMigrationOriginPolicy, "ALLOW"),
        ),
        FieldSpec::new(
            "execution_limits",
            FieldDefault::NativeCall(RecordId::ExecutionLimitsConfig),
        ),
        FieldSpec::new("diff", FieldDefault::NativeCall(RecordId::TargetDiffConfig)),
        FieldSpec::new(
            "warehouses",
            FieldDefault::NativeCall(RecordId::TargetWarehousesConfig),
        ),
        FieldSpec::new("concurrency", FieldDefault::None),
        FieldSpec::new("catalog_connection", FieldDefault::None),
    ],
};
const R_LOCAL_TARGET_CONFIG: RecordSpec = RecordSpec {
    name: "LocalTargetConfig",
    doc: "Local developer overrides for one named target.",
    fields: &[
        FieldSpec::new("connection", FieldDefault::EmptyDict),
        FieldSpec::new("connection_name", FieldDefault::None),
        FieldSpec::new("vars", FieldDefault::EmptyDict),
        FieldSpec::new("database", FieldDefault::None),
        FieldSpec::new("schema", FieldDefault::None),
        FieldSpec::new("loader_schema", FieldDefault::None),
        FieldSpec::new("defer_sources_to", FieldDefault::None),
        FieldSpec::new("defer_clone_from", FieldDefault::None),
        FieldSpec::new(
            "clone",
            FieldDefault::NativeCall(RecordId::LocalClonePolicy),
        ),
        FieldSpec::new("compile_cache", FieldDefault::None),
        FieldSpec::new("time_travel_retention", FieldDefault::None),
        FieldSpec::new(
            "time_travel_retention_by_materialization",
            FieldDefault::None,
        ),
        FieldSpec::new("owns_time_travel_retention_namespace", FieldDefault::None),
        FieldSpec::new("default_table_type", FieldDefault::None),
        FieldSpec::new("table_type_downgrade", FieldDefault::None),
        FieldSpec::new("time_travel_retention_decrease", FieldDefault::None),
        FieldSpec::new("missing_migration_origin", FieldDefault::None),
        FieldSpec::new(
            "execution_limits",
            FieldDefault::NativeCall(RecordId::ExecutionLimitsConfig),
        ),
        FieldSpec::new("diff", FieldDefault::NativeCall(RecordId::TargetDiffConfig)),
        FieldSpec::new(
            "warehouses",
            FieldDefault::NativeCall(RecordId::TargetWarehousesConfig),
        ),
        FieldSpec::new("concurrency", FieldDefault::None),
        FieldSpec::new("catalog_connection", FieldDefault::None),
    ],
};
const R_SETTINGS_CONFIG: RecordSpec = RecordSpec {
    name: "SettingsConfig",
    doc: "Global feature toggles.",
    fields: &[
        FieldSpec::new("sql_analysis", FieldDefault::True),
        FieldSpec::new("require_sql_analysis", FieldDefault::False),
        FieldSpec::new("query_change_tracking", FieldDefault::True),
        FieldSpec::new("check_declared_columns", FieldDefault::True),
        FieldSpec::new("concurrency", FieldDefault::Int(1)),
        FieldSpec::new("auto_load_sources", FieldDefault::True),
        FieldSpec::new("microbatch_concurrency", FieldDefault::False),
        FieldSpec::new(
            "microbatch_unaccounted_partition_policy",
            FieldDefault::Text("synthesize"),
        ),
        FieldSpec::new("table_promotion_mode", FieldDefault::None),
        FieldSpec::new("default_audit_severity", FieldDefault::None),
        FieldSpec::new("default_audit_run_scope", FieldDefault::None),
        FieldSpec::new("unit_test_max_real_models", FieldDefault::Int(10)),
        FieldSpec::new("test_timeout", FieldDefault::Text("30s")),
        FieldSpec::new("test_timeout_seconds", FieldDefault::Int(30)),
        FieldSpec::new("scenario_timeout", FieldDefault::None),
        FieldSpec::new("scenario_timeout_seconds", FieldDefault::None),
        FieldSpec::new("delta_compile", FieldDefault::True),
    ],
};
const R_SCOPES_CONFIG: RecordSpec = RecordSpec {
    name: "ScopesConfig",
    doc: "Declaration scope policy configuration.",
    fields: &[FieldSpec::new("enforce_placement", FieldDefault::True)],
};
const R_REFERENCES_CONFIG: RecordSpec = RecordSpec {
    name: "ReferencesConfig",
    doc: "Explicit-reference enforcement policy configuration.",
    fields: &[FieldSpec::new("enforce_explicit", FieldDefault::True)],
};
const R_COST_CONFIG: RecordSpec = RecordSpec {
    name: "CostConfig",
    doc: "Snowflake compute estimate configuration.",
    fields: &[
        FieldSpec::new(
            "usd_per_credit",
            FieldDefault::CallText("decimal", "Decimal", "3.00"),
        ),
        FieldSpec::new("usd_per_credit_is_default", FieldDefault::True),
    ],
};
const R_CONSTANTS_CONFIG: RecordSpec = RecordSpec {
    name: "ConstantsConfig",
    doc: "Project-wide constant rendering configuration.",
    fields: &[FieldSpec::new(
        "collection_rendering",
        FieldDefault::NativeMember(RecordId::CollectionRendering, "VALUE_LIST"),
    )],
};
const R_FUTURE_CURSORS_CONFIG: RecordSpec = RecordSpec {
    name: "FutureCursorsConfig",
    doc: "Safety policy for automatically discovered future cursor watermarks.",
    fields: &[
        FieldSpec::new("max_distance", FieldDefault::None),
        FieldSpec::new(
            "action",
            FieldDefault::NativeMember(RecordId::FutureCursorAction, "ERROR"),
        ),
    ],
};
const R_START_CURSORS_CONFIG: RecordSpec = RecordSpec {
    name: "StartCursorsConfig",
    doc: "Safety policy for automatically discovered incremental starts.",
    fields: &[
        FieldSpec::new("max_ahead", FieldDefault::None),
        FieldSpec::new(
            "action",
            FieldDefault::NativeMember(RecordId::FutureCursorAction, "ERROR"),
        ),
    ],
};
const R_CURSORS_CONFIG: RecordSpec = RecordSpec {
    name: "CursorsConfig",
    doc: "Project-wide cursor safety configuration.",
    fields: &[
        FieldSpec::new(
            "future",
            FieldDefault::NativeCall(RecordId::FutureCursorsConfig),
        ),
        FieldSpec::new(
            "start",
            FieldDefault::NativeCall(RecordId::StartCursorsConfig),
        ),
    ],
};
const R_MICROBATCH_LIMITS_CONFIG: RecordSpec = RecordSpec {
    name: "MicrobatchLimitsConfig",
    doc: "Per-model microbatch planning safety limit.",
    fields: &[
        FieldSpec::new("max_batches", FieldDefault::None),
        FieldSpec::new(
            "action",
            FieldDefault::NativeMember(RecordId::MicrobatchLimitAction, "ERROR"),
        ),
    ],
};
const R_MICROBATCHES_CONFIG: RecordSpec = RecordSpec {
    name: "MicrobatchesConfig",
    doc: "Project-wide microbatch configuration.",
    fields: &[FieldSpec::new(
        "limits",
        FieldDefault::NativeCall(RecordId::MicrobatchLimitsConfig),
    )],
};
const R_DEFAULTS_CONFIG: RecordSpec = RecordSpec {
    name: "DefaultsConfig",
    doc: "Project-wide resource defaults.",
    fields: &[
        FieldSpec::new("materialized", FieldDefault::None),
        FieldSpec::new("database", FieldDefault::None),
        FieldSpec::new("schema", FieldDefault::None),
        FieldSpec::new("seed_database", FieldDefault::None),
        FieldSpec::new("seed_schema", FieldDefault::None),
        FieldSpec::new("function_database", FieldDefault::None),
        FieldSpec::new("function_schema", FieldDefault::None),
        FieldSpec::new("incremental_strategy", FieldDefault::None),
        FieldSpec::new("incremental_mode", FieldDefault::None),
        FieldSpec::new("microbatch_strategy", FieldDefault::None),
        FieldSpec::new("cursor_watermark_mode", FieldDefault::None),
        FieldSpec::new("merge_exclude_columns", FieldDefault::EmptyTuple),
        FieldSpec::new("full_refresh", FieldDefault::None),
        FieldSpec::new("append_cursor_inclusive", FieldDefault::None),
        FieldSpec::new("cursor_start", FieldDefault::None),
        FieldSpec::new("cursor_end", FieldDefault::None),
        FieldSpec::new("cursor_start_max_ahead", FieldDefault::None),
        FieldSpec::new("cursor_start_max_action", FieldDefault::None),
        FieldSpec::new("cursor_future_max_distance", FieldDefault::None),
        FieldSpec::new("cursor_future_action", FieldDefault::None),
        FieldSpec::new("lookback", FieldDefault::None),
        FieldSpec::new("batch_size", FieldDefault::None),
        FieldSpec::new("batch_concurrency", FieldDefault::None),
        FieldSpec::new("max_microbatches", FieldDefault::None),
        FieldSpec::new("max_duration", FieldDefault::None),
        FieldSpec::new("unaccounted_partition_policy", FieldDefault::None),
        FieldSpec::new("replay_on_change", FieldDefault::None),
        FieldSpec::new("row_diff_exclude_columns", FieldDefault::EmptyTuple),
        FieldSpec::new("row_diff_tolerances", FieldDefault::EmptyDict),
        FieldSpec::new("row_diff_sample_rows", FieldDefault::None),
        FieldSpec::new("row_diff_sample_seed", FieldDefault::None),
        FieldSpec::new("tags", FieldDefault::EmptyTuple),
        FieldSpec::new("contract", FieldDefault::None),
        FieldSpec::new("pre_hooks", FieldDefault::None),
        FieldSpec::new("post_hooks", FieldDefault::None),
    ],
};
const R_DIFF_CONFIG: RecordSpec = RecordSpec {
    name: "DiffConfig",
    doc: "Project-wide data-diff configuration.",
    fields: &[FieldSpec::new(
        "query_artifact_ttl",
        FieldDefault::Text("24h"),
    )],
};
const R_JANITOR_CONFIG: RecordSpec = RecordSpec {
    name: "JanitorConfig",
    doc: "Janitor command defaults.",
    fields: &[
        FieldSpec::new("enabled", FieldDefault::False),
        FieldSpec::new("retention_days", FieldDefault::None),
        FieldSpec::new("archive_retention_days", FieldDefault::Int(14)),
        FieldSpec::new("direct_state_history_versions", FieldDefault::Int(20)),
        FieldSpec::new("delete_tracked_only", FieldDefault::True),
        FieldSpec::new("exclude_patterns", FieldDefault::EmptyTuple),
        FieldSpec::new("in_use_window", FieldDefault::Text("7d")),
        FieldSpec::new("keep_in_use", FieldDefault::False),
    ],
};
const R_MIGRATIONS_CONFIG: RecordSpec = RecordSpec {
    name: "MigrationsConfig",
    doc: "Model migration defaults.",
    fields: &[FieldSpec::new("old_name_views", FieldDefault::Text("30d"))],
};
const R_SNAPSHOTS_CONFIG: RecordSpec = RecordSpec {
    name: "SnapshotsConfig",
    doc: "Snapshot materialization safety defaults.",
    fields: &[
        FieldSpec::new("current_state_full_refresh", FieldDefault::Text("deny")),
        FieldSpec::new(
            "historical_full_refresh",
            FieldDefault::Text("require_confirmation"),
        ),
        FieldSpec::new("schema_change", FieldDefault::Text("append_new_columns")),
        FieldSpec::new(
            "wildcard_check_schema_change",
            FieldDefault::Text("require_confirmation"),
        ),
    ],
};
const R_SCENARIO_SNAPSHOT_LIMITS_CONFIG: RecordSpec = RecordSpec {
    name: "ScenarioSnapshotLimitsConfig",
    doc: "Scenario snapshot capture safety limits.",
    fields: &[
        FieldSpec::new("max_rows_per_relation", FieldDefault::None),
        FieldSpec::new("max_total_rows", FieldDefault::None),
        FieldSpec::new("max_bytes_per_relation", FieldDefault::None),
        FieldSpec::new("max_total_bytes", FieldDefault::None),
    ],
};
const R_SCENARIO_CONFIG: RecordSpec = RecordSpec {
    name: "ScenarioConfig",
    doc: "Scenario command configuration.",
    fields: &[
        FieldSpec::new("run_namespace", FieldDefault::None),
        FieldSpec::new("local_type_overrides", FieldDefault::EmptyDict),
        FieldSpec::new(
            "snapshot_limits",
            FieldDefault::NativeCall(RecordId::ScenarioSnapshotLimitsConfig),
        ),
    ],
};
const R_DBT_CONFIG: RecordSpec = RecordSpec {
    name: "DbtConfig",
    doc: "dbt interop configuration.",
    fields: &[
        FieldSpec::new("project_dir", FieldDefault::None),
        FieldSpec::new("profiles_dir", FieldDefault::None),
        FieldSpec::new("target", FieldDefault::None),
        FieldSpec::new("target_path", FieldDefault::None),
        FieldSpec::new("vars", FieldDefault::EmptyDict),
    ],
};
const R_LOCAL_DBT_CONFIG: RecordSpec = RecordSpec {
    name: "LocalDbtConfig",
    doc: "Local dbt interop configuration overrides.",
    fields: &[
        FieldSpec::new("target", FieldDefault::None),
        FieldSpec::new("vars", FieldDefault::EmptyDict),
    ],
};
const R_LIFECYCLE_EVENT_SINK_FILTER_CONFIG: RecordSpec = RecordSpec {
    name: "LifecycleEventSinkFilterConfig",
    doc: "Destination-neutral runtime narrowing for lifecycle-event sinks.",
    fields: &[
        FieldSpec::new("event_kinds", FieldDefault::None),
        FieldSpec::new("min_severity", FieldDefault::None),
    ],
};
const R_LIFECYCLE_EVENT_SINKS_CONFIG: RecordSpec = RecordSpec {
    name: "LifecycleEventSinksConfig",
    doc: "Global and named lifecycle-event sink runtime filters and delivery settings.",
    fields: &[
        FieldSpec::new(
            "defaults",
            FieldDefault::NativeCall(RecordId::LifecycleEventSinkFilterConfig),
        ),
        FieldSpec::new("named", FieldDefault::EmptyDict),
        FieldSpec::new("shutdown_timeout", FieldDefault::None),
        FieldSpec::new("shutdown_timeout_seconds", FieldDefault::None),
    ],
};
const R_SINKS_CONFIG: RecordSpec = RecordSpec {
    name: "SinksConfig",
    doc: "Typed destination sink configuration.",
    fields: &[FieldSpec::new(
        "lifecycle",
        FieldDefault::NativeCall(RecordId::LifecycleEventSinksConfig),
    )],
};
const R_PROJECT_CONFIG: RecordSpec = RecordSpec {
    name: "ProjectConfig",
    doc: "Shared project configuration loaded from sqlbuild_project.toml.",
    fields: &[
        FieldSpec::required("name"),
        FieldSpec::required("adapter"),
        FieldSpec::new("default_target", FieldDefault::None),
        FieldSpec::new("connection", FieldDefault::EmptyDict),
        FieldSpec::new("connections", FieldDefault::EmptyDict),
        FieldSpec::new(
            "settings",
            FieldDefault::NativeCall(RecordId::SettingsConfig),
        ),
        FieldSpec::new("scopes", FieldDefault::NativeCall(RecordId::ScopesConfig)),
        FieldSpec::new(
            "references",
            FieldDefault::NativeCall(RecordId::ReferencesConfig),
        ),
        FieldSpec::new("cost", FieldDefault::NativeCall(RecordId::CostConfig)),
        FieldSpec::new(
            "constants",
            FieldDefault::NativeCall(RecordId::ConstantsConfig),
        ),
        FieldSpec::new("cursors", FieldDefault::NativeCall(RecordId::CursorsConfig)),
        FieldSpec::new(
            "microbatches",
            FieldDefault::NativeCall(RecordId::MicrobatchesConfig),
        ),
        FieldSpec::new(
            "defaults",
            FieldDefault::NativeCall(RecordId::DefaultsConfig),
        ),
        FieldSpec::new("diff", FieldDefault::NativeCall(RecordId::DiffConfig)),
        FieldSpec::new(
            "materialization_defaults",
            FieldDefault::NativeCall(RecordId::MaterializationDefaultsConfig),
        ),
        FieldSpec::new("path_defaults", FieldDefault::EmptyDict),
        FieldSpec::new("vars", FieldDefault::EmptyDict),
        FieldSpec::new("targets", FieldDefault::EmptyDict),
        FieldSpec::new("janitor", FieldDefault::NativeCall(RecordId::JanitorConfig)),
        FieldSpec::new(
            "migrations",
            FieldDefault::NativeCall(RecordId::MigrationsConfig),
        ),
        FieldSpec::new(
            "snapshots",
            FieldDefault::NativeCall(RecordId::SnapshotsConfig),
        ),
        FieldSpec::new(
            "scenario",
            FieldDefault::NativeCall(RecordId::ScenarioConfig),
        ),
        FieldSpec::new("dbt", FieldDefault::NativeCall(RecordId::DbtConfig)),
        FieldSpec::new("sinks", FieldDefault::NativeCall(RecordId::SinksConfig)),
    ],
};
const R_LOCAL_CONFIG: RecordSpec = RecordSpec {
    name: "LocalConfig",
    doc: "Local developer overrides from sqlbuild_local.toml.",
    fields: &[
        FieldSpec::new("target", FieldDefault::None),
        FieldSpec::new("adapter", FieldDefault::None),
        FieldSpec::new("connection", FieldDefault::EmptyDict),
        FieldSpec::new("connections", FieldDefault::EmptyDict),
        FieldSpec::new("targets", FieldDefault::EmptyDict),
        FieldSpec::new(
            "settings",
            FieldDefault::NativeCall(RecordId::SettingsConfig),
        ),
        FieldSpec::new("setting_overrides", FieldDefault::EmptyFrozenset),
        FieldSpec::new("vars", FieldDefault::EmptyDict),
        FieldSpec::new("dbt", FieldDefault::NativeCall(RecordId::LocalDbtConfig)),
        FieldSpec::new(
            "scenario",
            FieldDefault::NativeCall(RecordId::ScenarioConfig),
        ),
    ],
};
const R_SOURCE_LOCATION: RecordSpec = RecordSpec {
    name: "SourceLocation",
    doc: "An authored source location for compiler diagnostics.",
    fields: &[
        FieldSpec::required("path"),
        FieldSpec::required("line"),
        FieldSpec::required("column"),
        FieldSpec::new("end_line", FieldDefault::None),
        FieldSpec::new("end_column", FieldDefault::None),
    ],
};
const R_SEED_CSV_SETTINGS: RecordSpec = RecordSpec {
    name: "SeedCsvSettings",
    doc: "CSV reader settings for a seed file.",
    fields: &[
        FieldSpec::new("delimiter", FieldDefault::None),
        FieldSpec::new("quotechar", FieldDefault::None),
        FieldSpec::new("doublequote", FieldDefault::None),
        FieldSpec::new("escapechar", FieldDefault::None),
        FieldSpec::new("skipinitialspace", FieldDefault::None),
        FieldSpec::new("lineterminator", FieldDefault::None),
        FieldSpec::new("encoding", FieldDefault::None),
        FieldSpec::new("na_values", FieldDefault::None),
        FieldSpec::new("keep_default_na", FieldDefault::None),
    ],
};
const R_SCHEMA_AUDIT_INSTANCE: RecordSpec = RecordSpec {
    name: "SchemaAuditInstance",
    doc: "One audit instance attached to model, column, or seed metadata.",
    fields: &[
        FieldSpec::required("definition_name"),
        FieldSpec::new("arguments", FieldDefault::EmptyDict),
        FieldSpec::new("name", FieldDefault::None),
        FieldSpec::new("description", FieldDefault::None),
        FieldSpec::new("severity", FieldDefault::None),
        FieldSpec::new("run_scope", FieldDefault::None),
        FieldSpec::new("always_run", FieldDefault::False),
        FieldSpec::new("thresholds", FieldDefault::None),
        FieldSpec::new("minimum_samples", FieldDefault::None),
        FieldSpec::new("evidence_limit", FieldDefault::None),
        FieldSpec::new("meta", FieldDefault::None),
        FieldSpec {
            name: "location",
            default: FieldDefault::None,
            compared: false,
            shown: false,
        },
    ],
};
const R_SCHEMA_COLUMN: RecordSpec = RecordSpec {
    name: "SchemaColumn",
    doc: "One declared model, seed, or source column entry.",
    fields: &[
        FieldSpec::required("name"),
        FieldSpec::new("type", FieldDefault::None),
        FieldSpec::new("nullable", FieldDefault::None),
        FieldSpec::new("description", FieldDefault::None),
        FieldSpec::new("meta", FieldDefault::EmptyDict),
        FieldSpec::new("audits", FieldDefault::EmptyTuple),
        FieldSpec::new("location", FieldDefault::None),
        FieldSpec::new("migrate_from", FieldDefault::None),
    ],
};
const R_SCHEMA_DYNAMIC_COLUMN_FAMILY: RecordSpec = RecordSpec {
    name: "SchemaDynamicColumnFamily",
    doc: "One declared family of columns generated by a runtime-dynamic pivot.",
    fields: &[
        FieldSpec::required("name"),
        FieldSpec::required("pivot_column"),
        FieldSpec::required("value_column"),
        FieldSpec::required("aggregate"),
        FieldSpec::required("type"),
        FieldSpec::new("name_pattern", FieldDefault::None),
    ],
};
const R_SCHEMA_MODEL_ENTRY: RecordSpec = RecordSpec {
    name: "SchemaModelEntry",
    doc: "One model metadata entry normalized from MODEL(...).",
    fields: &[
        FieldSpec::required("name"),
        FieldSpec::new("model_schema", FieldDefault::None),
        FieldSpec::new("description", FieldDefault::None),
        FieldSpec::new("type_enforcement", FieldDefault::None),
        FieldSpec::new("meta", FieldDefault::EmptyDict),
        FieldSpec::new("columns", FieldDefault::EmptyTuple),
        FieldSpec::new("dynamic_columns", FieldDefault::EmptyTuple),
        FieldSpec::new("audits", FieldDefault::EmptyTuple),
        FieldSpec::new("tags", FieldDefault::EmptyTuple),
    ],
};
const R_SCHEMA_SEED_ENTRY: RecordSpec = RecordSpec {
    name: "SchemaSeedEntry",
    doc: "One seed metadata entry from seed YAML.",
    fields: &[
        FieldSpec::required("name"),
        FieldSpec::new("description", FieldDefault::None),
        FieldSpec::new("database", FieldDefault::None),
        FieldSpec::new("schema", FieldDefault::None),
        FieldSpec::new("meta", FieldDefault::EmptyDict),
        FieldSpec::new(
            "csv_settings",
            FieldDefault::NativeCall(RecordId::SeedCsvSettings),
        ),
        FieldSpec::new("columns", FieldDefault::EmptyTuple),
        FieldSpec::new("tags", FieldDefault::EmptyTuple),
        FieldSpec::new("audits", FieldDefault::EmptyTuple),
    ],
};
const R_INTEGRATION_LOADER_CONFIG: RecordSpec = RecordSpec {
    name: "IntegrationLoaderConfig",
    doc: "Typed configuration for one declarative integration loader.",
    fields: &[FieldSpec::required("kind"), FieldSpec::required("config")],
};
const R_SOURCE_FRESHNESS_AGE_POLICY: RecordSpec = RecordSpec {
    name: "SourceFreshnessAgePolicy",
    doc: "Age-based source freshness warning/error thresholds.",
    fields: &[
        FieldSpec::new("warn_after", FieldDefault::None),
        FieldSpec::new("error_after", FieldDefault::None),
    ],
};
const R_SOURCE_FRESHNESS_CONFIG: RecordSpec = RecordSpec {
    name: "SourceFreshnessConfig",
    doc: "Configured source freshness observation for planning.",
    fields: &[
        FieldSpec::required("strategy"),
        FieldSpec::new("value_kind", FieldDefault::None),
        FieldSpec::new("column", FieldDefault::None),
        FieldSpec::new("query", FieldDefault::None),
        FieldSpec::new("filter", FieldDefault::None),
        FieldSpec::new("lag_tolerance", FieldDefault::None),
        FieldSpec::new("age_policy", FieldDefault::None),
    ],
};
const R_SOURCE_COLUMN_ENTRY: RecordSpec = RecordSpec {
    name: "SourceColumnEntry",
    doc: "One source column entry from sources/*.yml.",
    fields: &[
        FieldSpec::required("name"),
        FieldSpec::new("type", FieldDefault::None),
        FieldSpec::new("nullable", FieldDefault::None),
        FieldSpec::new("description", FieldDefault::None),
        FieldSpec::new("meta", FieldDefault::EmptyDict),
        FieldSpec::new("audits", FieldDefault::EmptyTuple),
    ],
};
const R_SOURCE_ENTRY: RecordSpec = RecordSpec {
    name: "SourceEntry",
    doc: "One source declaration from sources/*.yml.",
    fields: &[
        FieldSpec::required("name"),
        FieldSpec::new("database", FieldDefault::None),
        FieldSpec::new("schema", FieldDefault::None),
        FieldSpec::new("table", FieldDefault::None),
        FieldSpec::new("final", FieldDefault::False),
        FieldSpec::new("loader", FieldDefault::None),
        FieldSpec::new("managed", FieldDefault::False),
        FieldSpec::new("integration_loader", FieldDefault::None),
        FieldSpec::new("freshness", FieldDefault::None),
        FieldSpec::new("write_strategy", FieldDefault::None),
        FieldSpec::new("load_batch_size", FieldDefault::None),
        FieldSpec::new("cursor_column", FieldDefault::None),
        FieldSpec::new("unique_key", FieldDefault::EmptyTuple),
        FieldSpec::new("expression", FieldDefault::None),
        FieldSpec::new("description", FieldDefault::None),
        FieldSpec::new("type_enforcement", FieldDefault::None),
        FieldSpec::new("contract", FieldDefault::None),
        FieldSpec::new("meta", FieldDefault::EmptyDict),
        FieldSpec::new("columns", FieldDefault::EmptyTuple),
        FieldSpec::new("audits", FieldDefault::EmptyTuple),
    ],
};
const R_LOADER_DESTINATION_PARTS: RecordSpec = RecordSpec {
    name: "LoaderDestinationParts",
    doc: "Parsed warehouse relation parts for a loader destination.",
    fields: &[
        FieldSpec::required("database"),
        FieldSpec::required("schema"),
        FieldSpec::required("table"),
    ],
};

/// The project configuration and contract value records.
pub(crate) const CONTRACT_RECORDS: &[RecordSpec] = &[
    R_CLONE_POLICY,
    R_LOCAL_CLONE_POLICY,
    R_AUTHORED_TIME_TRAVEL_RETENTION,
    R_MATERIALIZATION_RETENTION_DEFAULTS,
    R_MATERIALIZATION_DEFAULTS_CONFIG,
    R_RESOLVED_TIME_TRAVEL_RETENTION,
    R_RESOLVED_TABLE_TYPE,
    R_EXECUTION_LIMITS_CONFIG,
    R_TARGET_DIFF_CONFIG,
    R_TARGET_WAREHOUSES_CONFIG,
    R_TARGET_CONFIG,
    R_LOCAL_TARGET_CONFIG,
    R_SETTINGS_CONFIG,
    R_SCOPES_CONFIG,
    R_REFERENCES_CONFIG,
    R_COST_CONFIG,
    R_CONSTANTS_CONFIG,
    R_FUTURE_CURSORS_CONFIG,
    R_START_CURSORS_CONFIG,
    R_CURSORS_CONFIG,
    R_MICROBATCH_LIMITS_CONFIG,
    R_MICROBATCHES_CONFIG,
    R_DEFAULTS_CONFIG,
    R_DIFF_CONFIG,
    R_JANITOR_CONFIG,
    R_MIGRATIONS_CONFIG,
    R_SNAPSHOTS_CONFIG,
    R_SCENARIO_SNAPSHOT_LIMITS_CONFIG,
    R_SCENARIO_CONFIG,
    R_DBT_CONFIG,
    R_LOCAL_DBT_CONFIG,
    R_LIFECYCLE_EVENT_SINK_FILTER_CONFIG,
    R_LIFECYCLE_EVENT_SINKS_CONFIG,
    R_SINKS_CONFIG,
    R_PROJECT_CONFIG,
    R_LOCAL_CONFIG,
    R_SOURCE_LOCATION,
    R_SEED_CSV_SETTINGS,
    R_SCHEMA_AUDIT_INSTANCE,
    R_SCHEMA_COLUMN,
    R_SCHEMA_DYNAMIC_COLUMN_FAMILY,
    R_SCHEMA_MODEL_ENTRY,
    R_SCHEMA_SEED_ENTRY,
    R_INTEGRATION_LOADER_CONFIG,
    R_SOURCE_FRESHNESS_AGE_POLICY,
    R_SOURCE_FRESHNESS_CONFIG,
    R_SOURCE_COLUMN_ENTRY,
    R_SOURCE_ENTRY,
    R_LOADER_DESTINATION_PARTS,
];
