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

/// Physical namespace of SQLBuild's source-freshness state table, never a source query.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, utoipa::ToSchema,
)]
#[serde(deny_unknown_fields)]
pub struct WarehouseStateLocation {
    pub database: Option<String>,
    pub schema: String,
}

/// Only immutable history tables support a timestamp watermark.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum WarehouseHistoryKind {
    Fingerprints,
    SourceFreshness,
    AuditResults,
}

impl WarehouseHistoryKind {
    pub const ALL: [Self; 3] = [
        Self::Fingerprints,
        Self::SourceFreshness,
        Self::AuditResults,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fingerprints => "fingerprints",
            Self::SourceFreshness => "source_freshness",
            Self::AuditResults => "audit_results",
        }
    }

    #[must_use]
    pub const fn mirror_table(self) -> &'static str {
        match self {
            Self::Fingerprints => "sqb_warehouse_fingerprints",
            Self::SourceFreshness => "sqb_warehouse_freshness",
            Self::AuditResults => "sqb_warehouse_audit_results",
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

/// Duplicate result IDs may be appended; timestamps distinguish their ordered observations.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuditResultsRead {
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
    AuditResults(AuditResultsRead),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WarehouseHistoryRecord {
    SourceFreshness(WarehouseFreshnessRecord),
    Fingerprint(WarehouseFingerprintRecord),
    AuditResult(Box<WarehouseAuditRecord>),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WarehouseHistoryPage {
    pub records: Vec<WarehouseHistoryRecord>,
    pub next_after: Option<Vec<Option<String>>>,
}
