//! Published conditions and immutable attempts share one sensor abstraction.

use std::collections::BTreeSet;
use std::num::NonZeroU32;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::declarations::models::TriggerStatus;
use crate::platform::models::PlatformEventKind;
use crate::runs::models::{NodeOutcome, RunOutcome};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SensorCondition {
    PlatformEvent {
        kinds: BTreeSet<PlatformEventKind>,
    },
    RunEvent {
        job_key: String,
        kinds: BTreeSet<RunEventKind>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        executed_successfully: Option<BTreeSet<String>>,
    },
    NodeEvent {
        selection: BTreeSet<String>,
        kinds: BTreeSet<NodeEventKind>,
    },
    SourceNewData {
        selection: BTreeSet<String>,
    },
    SourceStale {
        selection: BTreeSet<String>,
        #[schema(value_type = u32, minimum = 1)]
        after_seconds: NonZeroU32,
    },
    Custom {
        function: String,
        code_hash: String,
    },
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, utoipa::ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum NodeEventKind {
    Success,
    Failed,
    Recovered,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, utoipa::ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum RunEventKind {
    Completed,
    Success,
    Warn,
    Partial,
    Failed,
    Cancelled,
}

impl RunEventKind {
    #[must_use]
    pub const fn matches(self, outcome: RunOutcome) -> bool {
        matches!(
            (self, outcome),
            (Self::Completed, _)
                | (Self::Success, RunOutcome::Success)
                | (Self::Warn, RunOutcome::Warn)
                | (Self::Partial, RunOutcome::Partial)
                | (Self::Failed, RunOutcome::Failed)
                | (Self::Cancelled, RunOutcome::Cancelled)
        )
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct TerminalSensorRun {
    pub(crate) run_id: Uuid,
    pub(crate) status: RunOutcome,
    pub(crate) last_event_storage_id: i64,
}

#[derive(Debug, Deserialize)]
pub(crate) struct NodeSensorEvent {
    pub(crate) run_id: Uuid,
    pub(crate) node_key: String,
    pub(crate) status: NodeOutcome,
    pub(crate) finished_storage_id: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SensorDefinition {
    pub description: String,
    pub job_key: String,
    pub when: SensorCondition,
    #[schema(value_type = u32, minimum = 1)]
    pub minimum_interval_seconds: NonZeroU32,
    #[serde(default)]
    pub default_status: TriggerStatus,
}

/// Storage order is authoritative even when a fact's occurrence time predates earlier pages.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SensorCursor {
    pub event_storage_id: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom: Option<String>,
}

/// Location-owned Python returns data; only the fenced scheduler may apply it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum CustomSensorOutcome {
    Completed {
        cursor: Option<String>,
        run_keys: Vec<String>,
    },
    Skipped {
        cursor: Option<String>,
        reason: String,
    },
    Failed,
    TimedOut,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SensorEvaluationRequest {
    pub evaluation_id: Uuid,
    pub trigger_id: Uuid,
    pub location_id: Uuid,
    pub target: String,
    pub snapshot_id: Uuid,
    pub definition: SensorDefinition,
    pub definition_fingerprint: String,
    pub cursor: SensorCursor,
    pub epoch: i64,
    pub requested_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "phase", rename_all = "snake_case", deny_unknown_fields)]
pub enum SensorEvaluationOutcome {
    Requested,
    Completed {
        cursor: SensorCursor,
        run_ids: Vec<Uuid>,
    },
    Failed {
        code: String,
    },
    TimedOut,
    Abandoned,
}

impl SensorEvaluationOutcome {
    #[must_use]
    pub const fn phase(&self) -> &'static str {
        match self {
            Self::Requested => "requested",
            Self::Completed { .. } => "completed",
            Self::Failed { .. } => "failed",
            Self::TimedOut => "timed_out",
            Self::Abandoned => "abandoned",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SensorEvaluationFact {
    pub request: SensorEvaluationRequest,
    pub outcome: SensorEvaluationOutcome,
}

#[derive(Debug, Deserialize)]
pub(crate) struct DueSensor {
    pub(crate) trigger_id: Uuid,
    pub(crate) location_id: Uuid,
    pub(crate) target: String,
    pub(crate) snapshot_id: Uuid,
    pub(crate) definition_json: SensorDefinition,
    pub(crate) status: TriggerStatus,
}

/// A sensor's committed position and recent checks, for a person who may list the target.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct PersonSensorState {
    /// Absent until the sensor first commits a cursor.
    pub cursor: Option<PersonSensorCursor>,
    /// Fingerprint of the published definition; a check with another one ran older code.
    pub definition_fingerprint: String,
    /// Newest first.
    pub evaluations: Vec<PersonSensorEvaluation>,
    /// Runs the sensor requested, by run key, newest first.
    pub run_requests: Vec<PersonSensorRunRequest>,
}

/// The control event position built-in conditions read from, and a custom sensor's own value after secret redaction.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct PersonSensorCursor {
    pub event_storage_id: i64,
    pub custom: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct PersonSensorEvaluation {
    pub evaluation_id: Uuid,
    /// The latest phase: `requested` alone means the check is in flight; otherwise `completed`, `failed`, `timed_out` or `abandoned`.
    pub phase: String,
    pub requested_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub cursor_before: PersonSensorCursor,
    /// Present when the check completed.
    pub cursor_after: Option<PersonSensorCursor>,
    pub run_count: usize,
    /// The failure code of a failed check.
    pub failure_code: Option<String>,
    pub definition_fingerprint: String,
}

/// A run key is redacted like the custom cursor it usually derives from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct PersonSensorRunRequest {
    pub run_key: String,
    pub run_id: Uuid,
    pub evaluation_id: Uuid,
    pub queued_at: DateTime<Utc>,
    pub status: crate::runs::models::RunStatus,
}

/// A published sensor and its committed cursor.
#[derive(Debug, Deserialize)]
pub(crate) struct StoredSensor {
    pub(crate) definition_json: SensorDefinition,
    pub(crate) cursor_json: Option<SensorCursor>,
}

/// One recorded phase of a sensor check.
#[derive(Debug, Deserialize)]
pub(crate) struct StoredEvaluationEvent {
    pub(crate) occurred_at: DateTime<Utc>,
    pub(crate) fact_json: SensorEvaluationFact,
}

/// A run key the sensor requested and the run it made.
#[derive(Debug, Deserialize)]
pub(crate) struct StoredRunRequest {
    pub(crate) run_key: String,
    pub(crate) run_id: Uuid,
    pub(crate) evaluation_id: Uuid,
    pub(crate) queued_at: DateTime<Utc>,
    pub(crate) status: crate::runs::models::RunStatus,
}
