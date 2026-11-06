//! Check results, their SQL and evidence, and maintenance history as people read them.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::warehouse_metadata::models::{
    WarehouseAuditRecord, WarehouseJanitorRecord, WarehouseMigrationRecord, WarehouseOldNameRecord,
};

/// One stored check result. Its SQL, error texts and evidence rows need their own permissions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CheckResult {
    pub event_id: Uuid,
    /// Identity of the check; a renamed model or changed severity or thresholds starts a new one.
    pub binding_key: String,
    pub audit_name: String,
    pub audit_definition_name: String,
    pub audit_description: Option<String>,
    pub attachment_kind: String,
    pub attached_target_kind: Option<String>,
    pub attached_target_name: Option<String>,
    pub attached_column_name: Option<String>,
    pub severity: String,
    pub evaluation_mode: String,
    pub run_scope_phase: String,
    pub thresholds_json: Option<String>,
    pub occurred_at: DateTime<Utc>,
    pub run_id: String,
    pub invocation_id: String,
    pub outcome: String,
    pub violation_count: Option<i64>,
    pub measured_value: Option<String>,
    pub sample_count: Option<i64>,
    pub sample_unit: Option<String>,
    pub minimum_samples: Option<i64>,
    pub evidence_count: Option<i64>,
    pub evidence_truncated: Option<String>,
    pub reused: String,
}

impl CheckResult {
    pub(crate) fn new(event_id: Uuid, record: WarehouseAuditRecord) -> Self {
        Self {
            event_id,
            binding_key: record.binding_key,
            audit_name: record.audit_name,
            audit_definition_name: record.audit_definition_name,
            audit_description: record.audit_description,
            attachment_kind: record.attachment_kind,
            attached_target_kind: record.attached_target_kind,
            attached_target_name: record.attached_target_name,
            attached_column_name: record.attached_column_name,
            severity: record.severity,
            evaluation_mode: record.evaluation_mode,
            run_scope_phase: record.run_scope_phase,
            thresholds_json: record.thresholds_json,
            occurred_at: record.occurred_at,
            run_id: record.run_id,
            invocation_id: record.invocation_id,
            outcome: record.outcome,
            violation_count: record.violation_count,
            measured_value: record.measured_value,
            sample_count: record.sample_count,
            sample_unit: record.sample_unit,
            minimum_samples: record.minimum_samples,
            evidence_count: record.evidence_count,
            evidence_truncated: record.evidence_truncated,
            reused: record.reused,
        }
    }
}

/// A check's latest SQL and error texts, which quote SQL; `view.sql` reads them, audited.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CheckSql {
    pub event_id: Uuid,
    pub binding_key: String,
    pub executed_sql: Option<String>,
    pub measurement_sql: Option<String>,
    pub evidence_sql: Option<String>,
    pub execution_error: Option<String>,
    pub evidence_error: Option<String>,
}

/// One result's stored evidence rows; `view.evidence` reads them, audited.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CheckEvidence {
    pub event_id: Uuid,
    pub binding_key: String,
    /// Every row key, in sorted order; the warehouse stores rows with sorted keys.
    pub columns: Vec<String>,
    /// Scalar cells aligned to `columns`; a row without a key holds null there.
    #[schema(value_type = Vec<Vec<Object>>)]
    pub rows: Vec<Vec<serde_json::Value>>,
    pub truncated: Option<String>,
}

/// Recorded moves, their old-name view steps and janitor cleanups, newest moves first.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct MaintenanceHistory {
    pub migrations: Vec<WarehouseMigrationRecord>,
    /// Every step recorded for the listed moves, oldest first; same-time steps are not in step order.
    pub old_name_views: Vec<WarehouseOldNameRecord>,
    /// Empty when the history is for one model; cleanups are not tied to models.
    pub janitor_events: Vec<WarehouseJanitorRecord>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaintenanceQuery {
    pub node: Option<String>,
    pub limit: Option<u32>,
}

/// How one recorded stretch of a microbatch model's cursor stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum BatchState {
    Complete,
    /// A hole between completed partitions.
    Gap,
    /// Completed at another version than the newest replay requirement covering it.
    ReplayRequired,
}

/// One stretch of the cursor, as raw bound texts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CoverageBatch {
    pub start: String,
    pub end: String,
    pub state: BatchState,
    /// The run that executed the latest completion here; absent for gaps and external completions.
    pub run_id: Option<String>,
}

/// Recorded coverage of one model's current relation; tracked only for concurrent microbatch models.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct MicrobatchCoverage {
    pub model: String,
    pub target_name: String,
    pub physical_generation_id: String,
    pub cursor_column: String,
    pub cursor_type: String,
    pub batch_size: String,
    /// Start of the earliest completed partition.
    pub earliest: String,
    /// End of the completed range that starts at `earliest` without a hole.
    pub contiguous_frontier: Option<String>,
    /// Holes between completed partitions, oldest first.
    pub known_gaps: Vec<CoverageBatch>,
    /// The newest batches, oldest first.
    pub batches: Vec<CoverageBatch>,
    /// Whether older batches were left out of `batches`.
    pub batches_truncated: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoverageQuery {
    pub limit: Option<u32>,
}
