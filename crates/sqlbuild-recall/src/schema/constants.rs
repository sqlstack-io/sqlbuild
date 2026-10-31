//! Recall table definitions, view names and schema versions.

use crate::schema::models::{ColumnSpec, ColumnType, Table, TableSpec};

/// The Recall schema version this build writes. Raise it when a column or view is added.
pub const RECALL_SCHEMA_VERSION: i64 = 5;
/// Text that only older schema versions wrote into ClickHouse view definitions, with the last version that wrote it. Version 3 changed only view bodies, so view columns cannot tell version 2 views, which set `join_use_nulls`, from current ones.
pub const SUPERSEDED_CLICKHOUSE_VIEW_MARKERS: [(&str, i64); 1] = [("join_use_nulls", 2)];
/// The oldest SQLite library the store's views run on (ordered `group_concat`).
pub const MINIMUM_SQLITE_VERSION: (u32, u32) = (3, 44);
/// The `recall_meta` component of schema rows.
pub const META_SCHEMA_COMPONENT: &str = "schema";
pub const META_SCHEMA_VERSION_KEY: &str = "schema_version";
/// The invocation ID of the schema version row an administrator's DDL script inserts.
pub const DDL_SCRIPT_INVOCATION: &str = "ddl-script";
/// The `recall_meta` component of maintenance claims.
pub const META_MAINTENANCE_COMPONENT: &str = "maintenance";
pub const META_MAINTENANCE_CLAIM_KEY: &str = "claim";

/// Assigned by storage rather than accepted from a producer's row values.
pub const INGESTED_AT_COLUMN: &str = "ingested_at";

/// Columns every Recall row carries, after the table's own columns.
pub const COMMON_COLUMNS: [ColumnSpec; 6] = [
    ColumnSpec::required("invocation_id", ColumnType::Text),
    ColumnSpec::required("invocation_started_at", ColumnType::Timestamp),
    ColumnSpec::required("schema_version", ColumnType::Integer),
    ColumnSpec::required("producer_version", ColumnType::Text),
    ColumnSpec::required(INGESTED_AT_COLUMN, ColumnType::Timestamp),
    ColumnSpec::required("content_hash", ColumnType::Text),
];
/// The artifact columns its content digest covers: what it is, not who saw it when.
pub const ARTIFACT_CONTENT_COLUMNS: [&str; 5] = ["digest", "kind", "media_type", "size", "content"];
/// Common columns left out of a row's content digest.
pub const UNHASHED_COLUMNS: [&str; 2] = ["ingested_at", "content_hash"];

const CORRELATIONS: [ColumnSpec; 5] = [
    ColumnSpec::optional("run_id", ColumnType::Text),
    ColumnSpec::optional("resource_id", ColumnType::Text),
    ColumnSpec::optional("resource_attempt_id", ColumnType::Text),
    ColumnSpec::optional("operation_id", ColumnType::Text),
    ColumnSpec::optional("statement_id", ColumnType::Text),
];

const INVOCATION_COLUMNS: [ColumnSpec; 24] = [
    ColumnSpec::optional("command", ColumnType::Text),
    ColumnSpec::optional("subcommand", ColumnType::Text),
    ColumnSpec::optional("selection", ColumnType::Json),
    ColumnSpec::optional("exclusion", ColumnType::Json),
    ColumnSpec::optional("project_name", ColumnType::Text),
    ColumnSpec::optional("project_dir_hash", ColumnType::Text),
    ColumnSpec::optional("target", ColumnType::Text),
    ColumnSpec::optional("adapter", ColumnType::Text),
    ColumnSpec::optional("destination_database", ColumnType::Text),
    ColumnSpec::optional("destination_schema", ColumnType::Text),
    ColumnSpec::optional("destination_fingerprint", ColumnType::Text),
    ColumnSpec::optional("sqlbuild_version", ColumnType::Text),
    ColumnSpec::optional("git_revision", ColumnType::Text),
    ColumnSpec::optional("git_dirty", ColumnType::Boolean),
    ColumnSpec::optional("host_token", ColumnType::Text),
    ColumnSpec::optional("pid", ColumnType::Integer),
    ColumnSpec::optional("external_context", ColumnType::Json),
    ColumnSpec::optional("recall_filter_active", ColumnType::Boolean),
    ColumnSpec::optional("argv_digest", ColumnType::Text),
    ColumnSpec::optional("origin", ColumnType::Text),
    ColumnSpec::optional("command_mode", ColumnType::Text),
    ColumnSpec::optional("selection_count", ColumnType::Integer),
    ColumnSpec::optional("selection_truncated", ColumnType::Boolean),
    ColumnSpec::optional("selector_files", ColumnType::Json),
];

const OUTPUT_COLUMNS: [ColumnSpec; 11] = [
    ColumnSpec::required("record_id", ColumnType::Text),
    ColumnSpec::required("sequence", ColumnType::Integer),
    ColumnSpec::required("stream", ColumnType::Text),
    ColumnSpec::required("occurred_at", ColumnType::Timestamp),
    ColumnSpec::required("data", ColumnType::Blob),
    ColumnSpec::required("byte_offset", ColumnType::Integer),
    ColumnSpec::required("byte_count", ColumnType::Integer),
    ColumnSpec::optional("priority", ColumnType::Text),
    ColumnSpec::required("dropped_before", ColumnType::Integer),
    ColumnSpec::optional("omitted_bytes", ColumnType::Integer),
    ColumnSpec::optional("omitted_sequences", ColumnType::Integer),
];

const EVENT_COLUMNS: [ColumnSpec; 29] = [
    ColumnSpec::required("event_id", ColumnType::Text),
    ColumnSpec::optional("invocation_sequence", ColumnType::Integer),
    ColumnSpec::required("occurred_at", ColumnType::Timestamp),
    ColumnSpec::required("event_type", ColumnType::Text),
    ColumnSpec::required("event_kind", ColumnType::Text),
    ColumnSpec::required("severity", ColumnType::Text),
    ColumnSpec::required("producer", ColumnType::Text),
    ColumnSpec::required("event_schema_version", ColumnType::Integer),
    CORRELATIONS[0],
    CORRELATIONS[1],
    CORRELATIONS[2],
    CORRELATIONS[3],
    CORRELATIONS[4],
    ColumnSpec::optional("command", ColumnType::Text),
    ColumnSpec::optional("resource_kind", ColumnType::Text),
    ColumnSpec::optional("resource_name", ColumnType::Text),
    ColumnSpec::optional("error_code", ColumnType::Text),
    ColumnSpec::optional("error_type", ColumnType::Text),
    ColumnSpec::optional("duration_ms", ColumnType::Real),
    ColumnSpec::optional("exit_code", ColumnType::Integer),
    ColumnSpec::optional("query_id", ColumnType::Text),
    ColumnSpec::optional("sql_digest", ColumnType::Text),
    ColumnSpec::optional("intent", ColumnType::Text),
    ColumnSpec::optional("statement_kind", ColumnType::Text),
    ColumnSpec::optional("purpose", ColumnType::Text),
    ColumnSpec::optional("outcome", ColumnType::Text),
    ColumnSpec::optional("row_count", ColumnType::Integer),
    ColumnSpec::optional("affected_rows", ColumnType::Integer),
    ColumnSpec::required("payload", ColumnType::Json),
];

const DIAGNOSTIC_COLUMNS: [ColumnSpec; 14] = [
    ColumnSpec::required("record_id", ColumnType::Text),
    ColumnSpec::required("sequence", ColumnType::Integer),
    ColumnSpec::required("occurred_at", ColumnType::Timestamp),
    ColumnSpec::required("severity", ColumnType::Text),
    ColumnSpec::required("logger", ColumnType::Text),
    ColumnSpec::required("source", ColumnType::Text),
    ColumnSpec::required("message", ColumnType::Text),
    ColumnSpec::required("fields", ColumnType::Json),
    ColumnSpec::optional("log_stream_id", ColumnType::Text),
    CORRELATIONS[0],
    CORRELATIONS[1],
    CORRELATIONS[2],
    CORRELATIONS[3],
    CORRELATIONS[4],
];

const ARTIFACT_COLUMNS: [ColumnSpec; 6] = [
    ColumnSpec::required("digest", ColumnType::Text),
    ColumnSpec::required("kind", ColumnType::Text),
    ColumnSpec::required("media_type", ColumnType::Text),
    ColumnSpec::required("size", ColumnType::Integer),
    ColumnSpec::required("content", ColumnType::Blob),
    ColumnSpec::required("seen_at", ColumnType::Timestamp),
];

const ARTIFACT_REF_COLUMNS: [ColumnSpec; 6] = [
    ColumnSpec::required("digest", ColumnType::Text),
    ColumnSpec::required("ref_id", ColumnType::Text),
    ColumnSpec::optional("statement_id", ColumnType::Text),
    ColumnSpec::optional("resource_id", ColumnType::Text),
    ColumnSpec::required("role", ColumnType::Text),
    ColumnSpec::required("referenced_at", ColumnType::Timestamp),
];

const HEARTBEAT_COLUMNS: [ColumnSpec; 6] = [
    ColumnSpec::required("beat_at", ColumnType::Timestamp),
    ColumnSpec::required("stale_at", ColumnType::Timestamp),
    ColumnSpec::optional("host_token", ColumnType::Text),
    ColumnSpec::optional("pid", ColumnType::Integer),
    ColumnSpec::optional("command", ColumnType::Text),
    ColumnSpec::optional("subcommand", ColumnType::Text),
];

const DELIVERY_COLUMNS: [ColumnSpec; 9] = [
    ColumnSpec::required("stream", ColumnType::Text),
    ColumnSpec::required("accepted", ColumnType::Integer),
    ColumnSpec::required("delivered", ColumnType::Integer),
    ColumnSpec::required("filtered", ColumnType::Integer),
    ColumnSpec::required("dropped", ColumnType::Integer),
    ColumnSpec::required("failed", ColumnType::Integer),
    ColumnSpec::required("conflicts", ColumnType::Integer),
    ColumnSpec::required("flush_complete", ColumnType::Boolean),
    ColumnSpec::required("written_at", ColumnType::Timestamp),
];

const META_COLUMNS: [ColumnSpec; 4] = [
    ColumnSpec::required("component", ColumnType::Text),
    ColumnSpec::required("key", ColumnType::Text),
    ColumnSpec::required("value", ColumnType::Text),
    ColumnSpec::required("recorded_at", ColumnType::Timestamp),
];

const CONFLICT_COLUMNS: [ColumnSpec; 5] = [
    ColumnSpec::required("dataset", ColumnType::Text),
    ColumnSpec::required("record_key", ColumnType::Text),
    ColumnSpec::required("existing_sha256", ColumnType::Text),
    ColumnSpec::required("rejected_sha256", ColumnType::Text),
    ColumnSpec::required("detected_at", ColumnType::Timestamp),
];

pub(crate) const INVOCATIONS_TABLE: TableSpec = TableSpec {
    name: "recall_invocations",
    clickhouse_version: None,
    columns: &INVOCATION_COLUMNS,
    key: &["invocation_id"],
    sqlite_indexes: &[("started", &["invocation_started_at"])],
    clickhouse_engine: REPLACING_ENGINE,
    clickhouse_order_by: &["invocation_started_at", "invocation_id"],
    ttl_extra_days: 0,
    expiry_column: "invocation_started_at",
};
pub(crate) const OUTPUT_TABLE: TableSpec = TableSpec {
    name: "recall_output",
    clickhouse_version: None,
    columns: &OUTPUT_COLUMNS,
    key: &["invocation_id", "sequence"],
    sqlite_indexes: &[("expiry", &["invocation_started_at"])],
    clickhouse_engine: REPLACING_ENGINE,
    clickhouse_order_by: &["invocation_id", "sequence"],
    ttl_extra_days: 0,
    expiry_column: "invocation_started_at",
};
pub(crate) const EVENTS_TABLE: TableSpec = TableSpec {
    name: "recall_events",
    clickhouse_version: None,
    columns: &EVENT_COLUMNS,
    key: &["event_id"],
    sqlite_indexes: &[
        ("invocation", &["invocation_id", "invocation_sequence"]),
        ("type", &["event_type", "occurred_at"]),
        ("statement", &["statement_id"]),
        ("expiry", &["invocation_started_at"]),
    ],
    clickhouse_engine: REPLACING_ENGINE,
    clickhouse_order_by: &["invocation_id", "event_id"],
    ttl_extra_days: 0,
    expiry_column: "invocation_started_at",
};
pub(crate) const DIAGNOSTICS_TABLE: TableSpec = TableSpec {
    name: "recall_diagnostics",
    clickhouse_version: None,
    columns: &DIAGNOSTIC_COLUMNS,
    key: &["invocation_id", "sequence"],
    sqlite_indexes: &[
        ("severity", &["severity"]),
        ("expiry", &["invocation_started_at"]),
    ],
    clickhouse_engine: REPLACING_ENGINE,
    clickhouse_order_by: &["invocation_id", "sequence"],
    ttl_extra_days: 0,
    expiry_column: "invocation_started_at",
};
pub(crate) const ARTIFACTS_TABLE: TableSpec = TableSpec {
    name: "recall_artifacts",
    clickhouse_version: Some("seen_at"),
    columns: &ARTIFACT_COLUMNS,
    key: &["digest"],
    sqlite_indexes: &[("seen", &["seen_at"])],
    clickhouse_engine: REPLACING_ENGINE,
    clickhouse_order_by: &["digest"],
    ttl_extra_days: ARTIFACT_REFRESH_DAYS,
    expiry_column: "seen_at",
};
pub(crate) const ARTIFACT_REFS_TABLE: TableSpec = TableSpec {
    name: "recall_artifact_refs",
    clickhouse_version: None,
    columns: &ARTIFACT_REF_COLUMNS,
    key: &["invocation_id", "digest", "ref_id"],
    sqlite_indexes: &[("digest", &["digest"])],
    clickhouse_engine: REPLACING_ENGINE,
    clickhouse_order_by: &["invocation_id", "digest", "ref_id"],
    ttl_extra_days: 0,
    expiry_column: "invocation_started_at",
};
pub(crate) const HEARTBEATS_TABLE: TableSpec = TableSpec {
    name: "recall_heartbeats",
    clickhouse_version: None,
    columns: &HEARTBEAT_COLUMNS,
    key: &["invocation_id", "beat_at"],
    sqlite_indexes: &[],
    clickhouse_engine: REPLACING_ENGINE,
    clickhouse_order_by: &["invocation_id", "beat_at"],
    ttl_extra_days: 0,
    expiry_column: "invocation_started_at",
};
pub(crate) const DELIVERY_TABLE: TableSpec = TableSpec {
    name: "recall_delivery",
    clickhouse_version: None,
    columns: &DELIVERY_COLUMNS,
    key: &["invocation_id", "stream"],
    sqlite_indexes: &[],
    clickhouse_engine: REPLACING_ENGINE,
    clickhouse_order_by: &["invocation_id", "stream"],
    ttl_extra_days: 0,
    expiry_column: "invocation_started_at",
};
pub(crate) const META_TABLE: TableSpec = TableSpec {
    name: "recall_meta",
    clickhouse_version: None,
    columns: &META_COLUMNS,
    key: &["component", "key", "recorded_at"],
    sqlite_indexes: &[],
    clickhouse_engine: "MergeTree",
    clickhouse_order_by: &["component", "key", "recorded_at"],
    ttl_extra_days: 0,
    expiry_column: "recorded_at",
};
pub(crate) const CONFLICTS_TABLE: TableSpec = TableSpec {
    name: "recall_conflicts",
    clickhouse_version: None,
    columns: &CONFLICT_COLUMNS,
    key: &["dataset", "record_key", "rejected_sha256"],
    sqlite_indexes: &[("invocation", &["invocation_id"])],
    clickhouse_engine: REPLACING_ENGINE,
    clickhouse_order_by: &["dataset", "record_key", "rejected_sha256"],
    ttl_extra_days: 0,
    expiry_column: "invocation_started_at",
};

/// Every table, in creation order.
pub const ALL_TABLES: [Table; 10] = [
    Table::Meta,
    Table::Invocations,
    Table::Output,
    Table::Events,
    Table::Diagnostics,
    Table::Artifacts,
    Table::ArtifactRefs,
    Table::Heartbeats,
    Table::Delivery,
    Table::Conflicts,
];
/// Tables whose rows belong to one invocation and expire with it.
pub const INVOCATION_TABLES: [Table; 8] = [
    Table::Invocations,
    Table::Output,
    Table::Events,
    Table::Diagnostics,
    Table::ArtifactRefs,
    Table::Heartbeats,
    Table::Delivery,
    Table::Conflicts,
];
/// The supported views, in creation order.
pub const VIEW_NAMES: [&str; 10] = [
    "invocations",
    "statements",
    "audit_results",
    "output_invocations",
    "output_tail",
    "output_transcripts",
    "events",
    "diagnostics",
    "artifacts",
    "conflicts",
];
/// The ClickHouse engine that collapses copies of a key; views read it with `FINAL`.
pub(crate) const REPLACING_ENGINE: &str = "ReplacingMergeTree";
/// The number of records `output_tail` keeps per invocation.
pub const OUTPUT_TAIL_RECORDS: u32 = 200;
/// Invocations deleted per maintenance transaction.
pub const RETENTION_BATCH_INVOCATIONS: u32 = 200;
/// ClickHouse re-inserts an artifact whose newest copy is older than this.
pub const ARTIFACT_REFRESH_DAYS: u32 = 7;
/// Tables whose expiry index finds candidate invocations for retention.
pub(crate) const RETENTION_CANDIDATE_TABLES: [Table; 5] = [
    Table::Invocations,
    Table::Heartbeats,
    Table::Events,
    Table::Output,
    Table::Diagnostics,
];

/// Seconds in a day, for comparing a version row's age with the retention.
pub(crate) const SECONDS_PER_DAY: i64 = 86_400;
