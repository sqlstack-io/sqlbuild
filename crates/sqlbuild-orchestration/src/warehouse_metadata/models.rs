//! Existing warehouse state rows, with their physical state-table provenance.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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

/// Receipt order is sensor progress; warehouse observation time remains source evidence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SourceFreshnessObservation {
    pub(crate) location_id: Uuid,
    pub(crate) target: String,
    pub(crate) record: WarehouseFreshnessRecord,
}

#[derive(Debug, Deserialize)]
pub(crate) struct StoredSourceFreshnessObservation {
    pub(crate) event_id: Uuid,
    pub(crate) location_id: Uuid,
    pub(crate) target: String,
    pub(crate) observed_at: DateTime<Utc>,
    pub(crate) record_json: WarehouseFreshnessRecord,
}

/// A bounded page from one registered reader of a published location/target generation.
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SourceFreshnessBatch {
    pub session_id: Uuid,
    pub generation: i64,
    pub records: Vec<WarehouseFreshnessRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reconciliation: Option<WarehouseReconciliationPage>,
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

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AuditResultsBatch {
    pub session_id: Uuid,
    pub generation: i64,
    pub records: Vec<WarehouseAuditRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reconciliation: Option<WarehouseReconciliationPage>,
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

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct JanitorEventsBatch {
    pub session_id: Uuid,
    pub generation: i64,
    pub records: Vec<WarehouseJanitorRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reconciliation: Option<WarehouseReconciliationPage>,
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

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct MigrationsBatch {
    pub session_id: Uuid,
    pub generation: i64,
    pub records: Vec<WarehouseMigrationRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reconciliation: Option<WarehouseReconciliationPage>,
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

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct OldNameViewsBatch {
    pub session_id: Uuid,
    pub generation: i64,
    pub records: Vec<WarehouseOldNameRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reconciliation: Option<WarehouseReconciliationPage>,
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

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct MicrobatchesBatch {
    pub session_id: Uuid,
    pub generation: i64,
    pub records: Vec<WarehouseMicrobatchRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reconciliation: Option<WarehouseReconciliationPage>,
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
    Catalog,
}

impl WarehouseHistoryKind {
    pub const ALL: [Self; 8] = [
        Self::Fingerprints,
        Self::SourceFreshness,
        Self::AuditResults,
        Self::JanitorEvents,
        Self::Migrations,
        Self::OldNameViews,
        Self::Microbatches,
        Self::Catalog,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fingerprints => "fingerprints",
            Self::SourceFreshness => "source_freshness",
            Self::AuditResults => "audit_results",
            Self::JanitorEvents => "janitor_events",
            Self::Migrations => "migrations",
            Self::OldNameViews => "old_name_views",
            Self::Microbatches => "microbatches",
            Self::Catalog => "catalog",
        }
    }

    #[must_use]
    pub const fn mirror_table(self) -> &'static str {
        match self {
            Self::Fingerprints => "sqb_warehouse_fingerprints",
            Self::SourceFreshness => "sqb_warehouse_freshness",
            Self::AuditResults => "sqb_warehouse_audit_results",
            Self::JanitorEvents => "sqb_warehouse_janitor_events",
            Self::Migrations => "sqb_warehouse_migrations",
            Self::OldNameViews => "sqb_warehouse_old_name_views",
            Self::Microbatches => "sqb_warehouse_microbatches",
            Self::Catalog => "sqb_warehouse_catalog",
        }
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

/// Private interpreter input; cursor values are data and receive adapter literal rendering.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceFreshnessRead {
    pub location: WarehouseStateLocation,
    pub after: Option<[Option<String>; 11]>,
    /// Inclusive time bound, held fixed across pages; independent of the row-key cursor.
    pub observed_since: Option<DateTime<Utc>>,
    pub limit: Option<usize>,
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

/// Fingerprint observations from one registered reader, independent of automatic polling.
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct FingerprintBatch {
    pub session_id: Uuid,
    pub generation: i64,
    pub records: Vec<WarehouseFingerprintRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reconciliation: Option<WarehouseReconciliationPage>,
}

/// A complete, ordered key-set scan of one physical state table, optionally one UTC day.
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct WarehouseReconciliationPage {
    pub scan_id: Uuid,
    pub page: u32,
    pub state_database: Option<String>,
    pub state_schema: String,
    pub day: Option<NaiveDate>,
    pub complete: bool,
    /// Repeat the identical final page with this mirror cursor until the response has none.
    pub after: Option<WarehouseReconciliationCursor>,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct WarehouseReconciliationCursor {
    pub observed_at: DateTime<Utc>,
    pub event_id: Uuid,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct WarehouseSyncResponse {
    pub applied: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_reconciliation_after: Option<WarehouseReconciliationCursor>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FingerprintRead {
    pub location: WarehouseStateLocation,
    pub after: Option<[Option<String>; 13]>,
    pub observed_since: Option<DateTime<Utc>>,
    pub limit: Option<usize>,
}

/// Stable event identities and timestamps order observations without comparing their payloads.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateEventsRead {
    pub location: WarehouseStateLocation,
    pub after: Option<[Option<String>; 3]>,
    pub observed_since: Option<DateTime<Utc>>,
    pub limit: Option<usize>,
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
    Catalog(CatalogRead),
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
    Catalog(WarehouseCatalogRecord),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WarehouseHistoryPage {
    pub records: Vec<WarehouseHistoryRecord>,
    pub next_after: Option<Vec<Option<String>>>,
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

#[derive(Debug)]
pub(crate) struct StateEventsImport<'a> {
    pub kind: WarehouseHistoryKind,
    pub session_id: Uuid,
    pub generation: i64,
    pub records: Vec<StateEventImport>,
    pub invalid_identity: bool,
    pub reconciliation: Option<&'a WarehouseReconciliationPage>,
    pub payload: serde_json::Value,
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

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CatalogBatch {
    pub session_id: Uuid,
    pub generation: i64,
    pub records: Vec<WarehouseCatalogRecord>,
    pub reconciliation: WarehouseReconciliationPage,
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
