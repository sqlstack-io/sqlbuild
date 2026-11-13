//! Existing warehouse state rows, with their physical state-table provenance.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use sqlbuild_core::constants::{
    AUDIT_RESULTS_TABLE_NAME, COLUMN_MIGRATION_TABLE_NAME, CONSUMPTION_TABLE_NAME,
    FINGERPRINT_TABLE_NAME, JANITOR_EVENTS_TABLE_NAME, MICROBATCH_TABLE_NAME, MIGRATION_TABLE_NAME,
    NODE_RESULTS_TABLE_NAME, OLD_NAME_VIEW_TABLE_NAME, SOURCE_FRESHNESS_TABLE_NAME,
};

use crate::store::errors::ControlError;
use crate::warehouse_metadata::constants::DEFAULT_STATISTICS_RETENTION_DAYS;

/// Preserve observations, not an inferred latest version, age status or run outcome.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct WarehouseFreshnessRecord {
    pub event_id: Option<String>,
    pub state_database: Option<String>,
    pub state_schema: String,
    pub source_name: String,
    pub target_database: Option<String>,
    pub target_schema: Option<String>,
    pub target_name: Option<String>,
    pub run_id: String,
    pub strategy: String,
    pub value_kind: String,
    pub data_version: Option<String>,
    pub data_version_hash: String,
    pub observed_at: DateTime<Utc>,
}

/// Preserve the warehouse representation, including text-encoded measurements and booleans.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct WarehouseAuditRecord {
    pub state_database: Option<String>,
    pub state_schema: String,
    pub result_id: String,
    pub schema_version: i64,
    pub occurred_at: DateTime<Utc>,
    pub invocation_id: String,
    pub run_id: String,
    pub audit_name: String,
    pub audit_definition_name: String,
    pub audit_description: Option<String>,
    pub binding_key: String,
    pub definition_fingerprint: String,
    pub execution_fingerprint: String,
    pub evaluation_mode: String,
    pub run_scope_phase: String,
    pub attachment_kind: String,
    pub attached_target_kind: Option<String>,
    pub attached_target_name: Option<String>,
    pub attached_column_name: Option<String>,
    pub target_database: Option<String>,
    pub target_schema: Option<String>,
    pub target_name: Option<String>,
    pub severity: String,
    pub outcome: String,
    pub execution_error: Option<String>,
    pub violation_count: Option<i64>,
    pub measured_value: Option<String>,
    pub sample_count: Option<i64>,
    pub sample_unit: Option<String>,
    pub minimum_samples: Option<i64>,
    pub thresholds_json: Option<String>,
    pub evidence_json: Option<String>,
    pub evidence_count: Option<i64>,
    pub evidence_truncated: Option<String>,
    pub evidence_error: Option<String>,
    pub measurement_sql: Option<String>,
    pub evidence_sql: Option<String>,
    pub executed_sql: Option<String>,
    pub sql_digest: Option<String>,
    pub metadata_json: Option<String>,
    pub reused: String,
    pub audit_meta_json: Option<String>,
}

/// Warehouse cleanup facts do not authorize control-plane deletion or worker recovery.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct WarehouseJanitorRecord {
    pub state_database: Option<String>,
    pub state_schema: String,
    pub event_id: String,
    pub schema_version: i64,
    pub event_type: String,
    pub occurred_at: DateTime<Utc>,
    pub run_id: String,
    pub relation_database: Option<String>,
    pub relation_schema: String,
    pub relation_type: Option<String>,
    pub original_name: Option<String>,
    pub original_qualified_name: Option<String>,
    pub archive_name: String,
    pub archive_qualified_name: String,
    pub archived_at: DateTime<Utc>,
}

/// Model-move history is evidence, not authority to migrate a warehouse relation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct WarehouseMigrationRecord {
    pub state_database: Option<String>,
    pub state_schema: String,
    pub event_id: String,
    pub target_name: Option<String>,
    pub origin_model: Option<String>,
    pub origin_database: Option<String>,
    pub origin_schema: Option<String>,
    pub origin_name: String,
    pub destination_model: String,
    pub destination_database: Option<String>,
    pub destination_schema: String,
    pub destination_name: String,
    pub origin_version_hash: String,
    pub discovery: String,
    pub decision: String,
    pub run_id: String,
    pub created_at: DateTime<Utc>,
}

/// Preserve nullable compatibility-view facts without projecting live warehouse existence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct WarehouseOldNameRecord {
    pub state_database: Option<String>,
    pub state_schema: String,
    pub event_id: String,
    pub target_name: Option<String>,
    pub event_type: String,
    pub migration_event_id: String,
    pub destination_model: String,
    pub old_database: Option<String>,
    pub old_schema: Option<String>,
    pub old_name: String,
    pub new_database: Option<String>,
    pub new_schema: String,
    pub new_name: String,
    pub view_retention: Option<String>,
    pub archive_name: Option<String>,
    pub column_aliases: Option<String>,
    pub grants_copied: Option<String>,
    pub view_sql: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub drop_reason: Option<String>,
    pub run_id: String,
    pub created_at: DateTime<Utc>,
}

/// Immutable coordination evidence, never authority to execute or mark a batch complete.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct WarehouseMicrobatchRecord {
    pub state_database: Option<String>,
    pub state_schema: String,
    pub event_id: String,
    pub record_type: String,
    pub scope_kind: String,
    pub scope_key: String,
    pub model_name: String,
    pub target_database: Option<String>,
    pub target_schema: Option<String>,
    pub target_name: String,
    pub physical_generation_id: String,
    pub origin_run_id: String,
    pub origin_run_started_at: Option<DateTime<Utc>>,
    pub execution_run_id: String,
    pub execution_run_started_at: Option<DateTime<Utc>>,
    pub run_type: String,
    pub completion_type: Option<String>,
    pub run_start: String,
    pub run_end: String,
    pub partition_start: Option<String>,
    pub partition_end: Option<String>,
    pub batch_size: String,
    pub cursor_column: String,
    pub cursor_type: String,
    pub cursor_grain: Option<String>,
    pub model_version_hash: Option<String>,
    pub definition_hash: Option<String>,
    pub fingerprint_status: String,
    pub replay_requirement_id: Option<String>,
    pub required_model_version_hash: Option<String>,
    pub previous_model_version_hash: Option<String>,
    pub replay_policy: Option<String>,
    pub rows_affected: Option<i64>,
    pub completed_at: Option<DateTime<Utc>>,
    pub coverage_source: Option<String>,
    pub observed_row_count: Option<i64>,
    pub observed_at: Option<DateTime<Utc>>,
    pub synthetic_reason: Option<String>,
    pub unaccounted_policy: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Physical namespace of SQLBuild's source-freshness state table, never a source query.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, utoipa::ToSchema,
)]
#[serde(deny_unknown_fields)]
pub struct WarehouseStateLocation {
    pub database: Option<String>,
    pub schema: String,
}

/// Column renames a model migration decided; only the event identity and time are required.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct WarehouseColumnMigrationRecord {
    pub state_database: Option<String>,
    pub state_schema: String,
    pub event_id: String,
    pub target_name: Option<String>,
    pub model_name: Option<String>,
    pub relation_database: Option<String>,
    pub relation_schema: Option<String>,
    pub relation_name: Option<String>,
    pub origin_column: Option<String>,
    pub destination_column: Option<String>,
    pub discovery: Option<String>,
    pub decision: Option<String>,
    pub run_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// One node's recorded outcome in one run, with its encoded payloads kept as written.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct WarehouseNodeResultRecord {
    pub state_database: Option<String>,
    pub state_schema: String,
    pub node_type: Option<String>,
    pub node_name: Option<String>,
    pub target_database: Option<String>,
    pub target_schema: Option<String>,
    pub target_name: Option<String>,
    pub run_id: Option<String>,
    pub status: Option<String>,
    pub payload_json_b64: Option<String>,
    pub metadata_json_b64: Option<String>,
    pub error_message: Option<String>,
    pub materialized: Option<String>,
    pub ts: DateTime<Utc>,
}

/// One input version an execution consumed, or the row marking that the execution started.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct WarehouseConsumptionRecord {
    pub state_database: Option<String>,
    pub state_schema: String,
    pub execution_id: Option<String>,
    pub phase: Option<String>,
    pub run_id: Option<String>,
    pub node_type: Option<String>,
    pub node_name: Option<String>,
    pub target_database: Option<String>,
    pub target_schema: Option<String>,
    pub target_name: Option<String>,
    pub version_hash: Option<String>,
    pub input_count: Option<i64>,
    pub input_kind: Option<String>,
    pub input_name: Option<String>,
    pub input_version: Option<String>,
    pub recorded_at: DateTime<Utc>,
}

/// Catalog observations require full scans; only immutable history supports a watermark.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum WarehouseHistoryKind {
    Fingerprints,
    SourceFreshness,
    AuditResults,
    JanitorEvents,
    Migrations,
    OldNameViews,
    Microbatches,
    ColumnMigrations,
    NodeResults,
    Consumption,
    Catalog,
    Statistics,
}

impl WarehouseHistoryKind {
    /// Catalog and statistics stay last: a failed statistics read rewinds one kind, to catalog.
    pub const ALL: [Self; 12] = [
        Self::Fingerprints,
        Self::SourceFreshness,
        Self::AuditResults,
        Self::JanitorEvents,
        Self::Migrations,
        Self::OldNameViews,
        Self::Microbatches,
        Self::ColumnMigrations,
        Self::NodeResults,
        Self::Consumption,
        Self::Catalog,
        Self::Statistics,
    ];

    /// Wire name, control-store mirror table and warehouse state table of each kind.
    const fn names(self) -> (&'static str, &'static str, Option<&'static str>) {
        match self {
            Self::Fingerprints => (
                "fingerprints",
                "sqb_warehouse_fingerprints",
                Some(FINGERPRINT_TABLE_NAME),
            ),
            Self::SourceFreshness => (
                "source_freshness",
                "sqb_warehouse_freshness",
                Some(SOURCE_FRESHNESS_TABLE_NAME),
            ),
            Self::AuditResults => (
                "audit_results",
                "sqb_warehouse_audit_results",
                Some(AUDIT_RESULTS_TABLE_NAME),
            ),
            Self::JanitorEvents => (
                "janitor_events",
                "sqb_warehouse_janitor_events",
                Some(JANITOR_EVENTS_TABLE_NAME),
            ),
            Self::Migrations => (
                "migrations",
                "sqb_warehouse_migrations",
                Some(MIGRATION_TABLE_NAME),
            ),
            Self::OldNameViews => (
                "old_name_views",
                "sqb_warehouse_old_name_views",
                Some(OLD_NAME_VIEW_TABLE_NAME),
            ),
            Self::Microbatches => (
                "microbatches",
                "sqb_warehouse_microbatches",
                Some(MICROBATCH_TABLE_NAME),
            ),
            Self::ColumnMigrations => (
                "column_migrations",
                "sqb_warehouse_column_migrations",
                Some(COLUMN_MIGRATION_TABLE_NAME),
            ),
            Self::NodeResults => (
                "node_results",
                "sqb_warehouse_node_results",
                Some(NODE_RESULTS_TABLE_NAME),
            ),
            Self::Consumption => (
                "consumption",
                "sqb_warehouse_consumption",
                Some(CONSUMPTION_TABLE_NAME),
            ),
            Self::Catalog => ("catalog", "sqb_warehouse_catalog", None),
            Self::Statistics => ("statistics", "sqb_warehouse_statistics", None),
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.names().0
    }

    /// The state table a kind mirrors; catalog and statistics read the warehouse catalog instead.
    #[must_use]
    pub const fn source_table(self) -> Option<&'static str> {
        self.names().2
    }

    #[must_use]
    pub const fn mirror_table(self) -> &'static str {
        self.names().1
    }
}

/// Derive progress from committed mirror rows, scoped to the active physical reader.
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct WarehouseWatermarkRead {
    pub session_id: Uuid,
    pub generation: i64,
    pub location: WarehouseStateLocation,
    pub kind: WarehouseHistoryKind,
}

/// A registered reader asks whether a run ended since its last synchronization began.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct WarehouseSyncDueRead {
    pub session_id: Uuid,
    pub generation: i64,
}

/// Both times come from the server clock, so the reader compares them without clock skew.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct WarehouseSyncDue {
    pub runs_ended_at: Option<DateTime<Utc>>,
    pub now: DateTime<Utc>,
}

/// Private interpreter input; cursor values are data and receive adapter literal rendering.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceFreshnessRead {
    pub location: WarehouseStateLocation,
    pub after: Option<[Option<String>; 11]>,
    /// Inclusive time bound, held fixed across pages; independent of the row-key cursor.
    pub observed_since: Option<DateTime<Utc>>,
    pub limit: Option<usize>,
    /// The table's change token after the last complete read; an equal token skips this read.
    #[serde(default)]
    pub unchanged_token: Option<String>,
}

/// Fingerprints remain history here; latest-row selection belongs to the existing planner reader.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct WarehouseFingerprintRecord {
    pub event_id: Option<String>,
    pub state_database: Option<String>,
    pub state_schema: String,
    pub node_type: String,
    pub node_name: String,
    pub target_database: Option<String>,
    pub target_schema: Option<String>,
    pub target_name: Option<String>,
    pub run_id: String,
    pub definition_hash: String,
    pub version_hash: String,
    pub schema_fingerprint: String,
    pub definition: String,
    pub metadata_json: String,
    pub ts: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct WarehouseMirrorCursor {
    pub observed_at: DateTime<Utc>,
    pub event_id: Uuid,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct WarehouseSyncResponse {
    pub applied: bool,
}

/// One bounded page of one state table from the registered reader of a published generation.
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct MetadataBatch {
    pub kind: WarehouseHistoryKind,
    pub session_id: Uuid,
    pub generation: i64,
    #[schema(value_type = Vec<Object>)]
    pub records: Vec<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scan: Option<MetadataScan>,
}

/// A full scan's complete page removes the namespace's rows that the scan did not deliver.
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct MetadataScan {
    pub scan_id: Uuid,
    pub location: WarehouseStateLocation,
    pub complete: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FingerprintRead {
    pub location: WarehouseStateLocation,
    pub after: Option<[Option<String>; 13]>,
    pub observed_since: Option<DateTime<Utc>>,
    pub limit: Option<usize>,
    /// The table's change token after the last complete read; an equal token skips this read.
    #[serde(default)]
    pub unchanged_token: Option<String>,
}

/// Node results carry no event identity, so the cursor holds the row's identity columns.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeResultsRead {
    pub location: WarehouseStateLocation,
    pub after: Option<[Option<String>; 9]>,
    pub observed_since: Option<DateTime<Utc>>,
    pub limit: Option<usize>,
    /// The table's change token after the last complete read; an equal token skips this read.
    #[serde(default)]
    pub unchanged_token: Option<String>,
}

/// Consumption rows carry no event identity, so the cursor holds the row's identity columns.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConsumptionRead {
    pub location: WarehouseStateLocation,
    pub after: Option<[Option<String>; 7]>,
    pub observed_since: Option<DateTime<Utc>>,
    pub limit: Option<usize>,
    /// The table's change token after the last complete read; an equal token skips this read.
    #[serde(default)]
    pub unchanged_token: Option<String>,
}

/// Stable event identities and timestamps order observations without comparing their payloads.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateEventsRead {
    pub location: WarehouseStateLocation,
    pub after: Option<[Option<String>; 3]>,
    pub observed_since: Option<DateTime<Utc>>,
    pub limit: Option<usize>,
    /// The table's change token after the last complete read; an equal token skips this read.
    #[serde(default)]
    pub unchanged_token: Option<String>,
}

/// The private interpreter accepts only known state tables, never arbitrary SQL.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WarehouseHistoryRead {
    SourceFreshness(SourceFreshnessRead),
    Fingerprints(FingerprintRead),
    AuditResults(StateEventsRead),
    JanitorEvents(StateEventsRead),
    Migrations(StateEventsRead),
    OldNameViews(StateEventsRead),
    Microbatches(StateEventsRead),
    ColumnMigrations(StateEventsRead),
    NodeResults(NodeResultsRead),
    Consumption(ConsumptionRead),
    Catalog(CatalogRead),
    Statistics(StatisticsRead),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WarehouseHistoryRecord {
    SourceFreshness(WarehouseFreshnessRecord),
    Fingerprint(WarehouseFingerprintRecord),
    AuditResult(Box<WarehouseAuditRecord>),
    JanitorEvent(WarehouseJanitorRecord),
    Migration(Box<WarehouseMigrationRecord>),
    OldNameView(Box<WarehouseOldNameRecord>),
    Microbatch(Box<WarehouseMicrobatchRecord>),
    ColumnMigration(Box<WarehouseColumnMigrationRecord>),
    NodeResult(Box<WarehouseNodeResultRecord>),
    Consumption(Box<WarehouseConsumptionRecord>),
    Catalog(WarehouseCatalogRecord),
    Statistics(WarehouseStatisticsRecord),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WarehouseHistoryPage {
    pub records: Vec<WarehouseHistoryRecord>,
    pub next_after: Option<Vec<Option<String>>>,
    /// The table's change token, read before a scan's first page where the warehouse offers one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub change_token: Option<String>,
    /// The token still equals the request's: nothing was read and nothing is delivered.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub unchanged: bool,
}

/// Typed import columns are selected by code, never from location-supplied field names.
#[derive(Debug)]
pub(crate) struct StateEventImport {
    pub source_id: String,
    pub location: WarehouseStateLocation,
    pub observed_at: DateTime<Utc>,
    pub columns: Vec<(&'static str, crate::store::models::Parameter)>,
    pub record_json: serde_json::Value,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct WarehouseCatalogColumn {
    pub name: String,
    pub data_type: String,
}

/// Physical identity and ordered columns come from the adapter catalog, not fingerprints.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct WarehouseCatalogRecord {
    pub state_database: Option<String>,
    pub state_schema: String,
    pub name: String,
    pub relation_type: String,
    pub columns: Vec<WarehouseCatalogColumn>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogRead {
    pub location: WarehouseStateLocation,
    pub after: Option<[Option<String>; 1]>,
    /// Catalog reads reject watermarks; the shared reader envelope supplies null.
    pub observed_since: Option<DateTime<Utc>>,
    pub limit: Option<usize>,
}

/// Read one exact physical namespace; an omitted database means SQL NULL, never a wildcard.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogQuery {
    pub database: Option<String>,
    pub schema: String,
    pub after_observed_at: Option<DateTime<Utc>>,
    pub after_event_id: Option<Uuid>,
    pub limit: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CatalogObservation {
    pub event_id: Uuid,
    pub observed_at: DateTime<Utc>,
    pub record: WarehouseCatalogRecord,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CatalogPage {
    pub records: Vec<CatalogObservation>,
    pub next_after: Option<WarehouseMirrorCursor>,
}

/// Exact node and physical destination, not a selector; omitted namespace values match SQL NULL.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FingerprintQuery {
    pub node_type: String,
    pub node_name: String,
    pub database: Option<String>,
    pub schema: Option<String>,
    pub name: Option<String>,
}

/// Build evidence only: no SQL, arbitrary metadata, or inference about live relation existence.
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct FingerprintObservation {
    pub event_id: Uuid,
    pub observed_at: DateTime<Utc>,
    pub source_event_id: String,
    pub run_id: String,
    pub definition_hash: String,
    pub version_hash: String,
    pub schema_fingerprint: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeStatusQuery {
    pub node_key: String,
}

/// Observed physical evidence is not a live warehouse-existence guarantee.
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum PhysicalEvidence {
    Observed { observation: CatalogObservation },
    Unavailable { reason: String },
}

/// Local definition identity only, not upstream data or graph currency.
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum DefinitionEvidence {
    Matched {
        local_hash: String,
    },
    Changed {
        current_local_hash: String,
        recorded_local_hash: String,
    },
    Unavailable {
        reason: String,
        current_local_hash: Option<String>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum SourceDataStatus {
    Unknown { reason: String },
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct NodeStatus {
    pub snapshot_id: Uuid,
    pub node_key: String,
    pub physical: PhysicalEvidence,
    pub definition: DefinitionEvidence,
    pub build: Option<FingerprintObservation>,
    pub source_data: SourceDataStatus,
}

/// A metadata estimate at collection time, never evidence of source-data currency.
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct WarehouseStatisticsRecord {
    pub state_database: Option<String>,
    pub state_schema: String,
    pub name: String,
    pub observed_at: DateTime<Utc>,
    pub row_count: Option<u64>,
    pub detail: Option<String>,
    pub byte_count: Option<u64>,
    pub byte_count_basis: Option<String>,
}

/// Eligibility comes from each relation's committed hour, never a namespace watermark.
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct StatisticsDueRead {
    pub session_id: Uuid,
    pub generation: i64,
    pub location: WarehouseStateLocation,
    pub after: Option<Uuid>,
    pub limit: usize,
}

/// One bounded catalog page; sampled and unsampled relations both advance its cursor.
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct StatisticsRead {
    pub location: WarehouseStateLocation,
    pub names: Vec<String>,
    pub next_after: Option<Uuid>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatisticsQuery {
    pub database: Option<String>,
    pub schema: String,
    pub name: String,
    pub after_observed_at: Option<DateTime<Utc>>,
    pub after_event_id: Option<Uuid>,
    pub limit: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct StatisticsObservation {
    pub event_id: Uuid,
    pub record: WarehouseStatisticsRecord,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct StatisticsPage {
    pub records: Vec<StatisticsObservation>,
    pub next_after: Option<WarehouseMirrorCursor>,
}

/// Installation configuration: expired samples are hidden on read before bounded pruning removes them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StatisticsRetention {
    days: u16,
}

impl StatisticsRetention {
    pub fn days(days: u16) -> Result<Self, ControlError> {
        if days == 0 {
            return Err(ControlError::Invalid(
                "statistics retention must keep at least one day".into(),
            ));
        }
        Ok(Self { days })
    }

    pub(crate) fn cutoff(self, now: DateTime<Utc>) -> DateTime<Utc> {
        now - Duration::days(i64::from(self.days))
    }

    pub(crate) fn kept_days(self) -> u16 {
        self.days
    }
}

impl Default for StatisticsRetention {
    fn default() -> Self {
        Self {
            days: DEFAULT_STATISTICS_RETENTION_DAYS,
        }
    }
}

/// A current snapshot node's declared metadata and latest build, without SQL, source or config.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CatalogNodeRecord {
    pub node_key: String,
    /// The snapshot node kind, such as `model`, `seed`, `source` or `audit`.
    pub kind: String,
    pub name: String,
    pub path: Option<String>,
    pub materialization: Option<String>,
    pub description: Option<String>,
    pub tags: Vec<String>,
    /// Keys of the nodes this node reads.
    pub dependencies: Vec<String>,
    /// The warehouse relation the node builds or reads.
    pub relation: Option<CatalogRelation>,
    pub column_count: Option<usize>,
    /// The newest fingerprint recorded for the node at its current relation, from any build.
    pub last_build: Option<CatalogNodeBuild>,
    /// The newest decisive build attempt, from scheduler runs or a newer build outside them.
    pub last_outcome: Option<CatalogNodeOutcome>,
    /// The current definition differs from the one `last_build` recorded; false when either is unknown.
    pub definition_changed: bool,
    pub snapshot_id: Uuid,
}

/// `success` or `failed`; `run_id` is the scheduler run, null for a newer build outside the scheduler.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CatalogNodeOutcome {
    pub status: String,
    pub at: DateTime<Utc>,
    pub run_id: Option<Uuid>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CatalogRelation {
    pub database: Option<String>,
    pub schema: String,
    pub name: String,
}

/// A fingerprint row's build identity; the `sqb` run that wrote it, not a scheduler run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CatalogNodeBuild {
    pub observed_at: DateTime<Utc>,
    pub run_id: String,
    pub version_hash: String,
}

/// A node's columns as its compiled definition infers them; `data_type` is absent when inference could not tell.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct LineageNodeColumns {
    pub node_key: String,
    pub kind: String,
    pub columns: Vec<LineageColumn>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct LineageColumn {
    pub name: String,
    pub data_type: Option<String>,
}

/// One published column lineage edge; `transform` is the compiler's transform kind, such as `direct` or `cast`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ColumnLineageEdge {
    pub source_key: String,
    pub source_column: String,
    pub target_key: String,
    pub target_column: String,
    pub transform: String,
}

/// The column lineage reaching or leaving one node, from the current snapshot; empty before one publishes lineage.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ColumnLineageRead {
    pub snapshot_id: Option<Uuid>,
    pub nodes: Vec<LineageNodeColumns>,
    pub edges: Vec<ColumnLineageEdge>,
}
