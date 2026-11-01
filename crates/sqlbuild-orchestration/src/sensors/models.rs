//! Published conditions and immutable attempts share one sensor abstraction.

use std::collections::BTreeSet;
use std::num::NonZeroU32;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::declarations::models::TriggerStatus;
use crate::platform::models::PlatformEventKind;
use crate::runs::models::RunOutcome;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SensorStatus {
    #[default]
    Running,
    Stopped,
}

impl SensorStatus {
    #[must_use]
    pub const fn projection(self) -> TriggerStatus {
        match self {
            Self::Running => TriggerStatus::Running,
            Self::Stopped => TriggerStatus::Paused,
        }
    }
}

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
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, utoipa::ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum NodeEventKind {
    Success,
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
    PresumedFailed,
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
                | (Self::PresumedFailed, RunOutcome::PresumedFailed)
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
    pub(crate) event_id: Uuid,
    pub(crate) run_id: Uuid,
    pub(crate) storage_id: i64,
    pub(crate) payload: serde_json::Value,
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
    pub default_status: SensorStatus,
}

/// Storage order is authoritative even when a fact's occurrence time predates earlier pages.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SensorCursor {
    pub event_storage_id: i64,
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
