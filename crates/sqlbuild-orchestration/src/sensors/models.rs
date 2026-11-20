//! Published conditions and immutable attempts share one sensor abstraction.

use std::collections::BTreeSet;
use std::num::NonZeroU32;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::declarations::models::{DeclaredAt, TriggerStatus};
use crate::platform::models::PlatformEventKind;
use crate::runs::models::{NodeOutcome, RunOutcome};

/// What wakes a sensor; a sensor without one polls every `minimum_interval_seconds`.
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
    /// Checks, audits and SQL tests that run inside runs, by node key.
    CheckEvent {
        selection: BTreeSet<String>,
        kinds: BTreeSet<CheckEventKind>,
    },
    SourceNewData {
        selection: BTreeSet<String>,
    },
    SourceStale {
        selection: BTreeSet<String>,
        #[schema(value_type = u32, minimum = 1)]
        after_seconds: NonZeroU32,
    },
}

/// How a check, audit or SQL test that ran inside a run fell short.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, utoipa::ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum CheckEventKind {
    Failed,
    Warn,
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
    /// A group member the group did not run because a job it depends on failed.
    Skipped,
}

impl RunEventKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Success => "success",
            Self::Warn => "warn",
            Self::Partial => "partial",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Skipped => "skipped",
        }
    }

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
                | (Self::Skipped, RunOutcome::Skipped)
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
    pub(crate) job_key: Option<String>,
    pub(crate) occurred_at: DateTime<Utc>,
}

/// A sensor: the function the location runs, and the condition that wakes it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SensorDefinition {
    pub description: String,
    /// `None` polls every `minimum_interval_seconds`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<SensorCondition>,
    /// The decorated function's name, such as `orders_failed`.
    pub function: String,
    pub code_hash: String,
    #[schema(value_type = u32, minimum = 1)]
    pub minimum_interval_seconds: NonZeroU32,
    /// How long one call of the function may take before its tick fails.
    #[schema(value_type = u32, minimum = 1)]
    pub timeout_seconds: NonZeroU32,
    #[serde(default)]
    pub default_status: TriggerStatus,
    /// The targets the declaration is limited to; empty for every target.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub targets: BTreeSet<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declared_at: Option<DeclaredAt>,
}

/// One event a sensor's function is called with; `id` is stable across redeliveries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SensorEvent {
    Run {
        id: String,
        occurred_at: DateTime<Utc>,
        job: String,
        run_id: Uuid,
        status: RunOutcome,
        error: Option<String>,
        failed_nodes: Vec<String>,
        failed_checks: Vec<String>,
        target: String,
        url: Option<String>,
    },
    Node {
        id: String,
        occurred_at: DateTime<Utc>,
        run_id: Uuid,
        job: Option<String>,
        node: String,
        event: NodeEventKind,
        target: String,
        url: Option<String>,
    },
    Check {
        id: String,
        occurred_at: DateTime<Utc>,
        run_id: Uuid,
        job: Option<String>,
        check: String,
        status: CheckEventKind,
        target: String,
        url: Option<String>,
    },
    SourceNewData {
        id: String,
        occurred_at: DateTime<Utc>,
        source: String,
        data_version: Option<String>,
        target: String,
    },
    SourceStale {
        id: String,
        occurred_at: DateTime<Utc>,
        source: String,
        data_version: Option<String>,
        after_seconds: u32,
        target: String,
    },
    Platform {
        id: String,
        event: PlatformEventKind,
        occurred_at: DateTime<Utc>,
        #[schema(value_type = Object)]
        details: serde_json::Value,
    },
}

impl SensorEvent {
    #[must_use]
    pub const fn occurred_at(&self) -> DateTime<Utc> {
        match self {
            Self::Run { occurred_at, .. }
            | Self::Node { occurred_at, .. }
            | Self::Check { occurred_at, .. }
            | Self::SourceNewData { occurred_at, .. }
            | Self::SourceStale { occurred_at, .. }
            | Self::Platform { occurred_at, .. } => *occurred_at,
        }
    }

    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Self::Run { id, .. }
            | Self::Node { id, .. }
            | Self::Check { id, .. }
            | Self::SourceNewData { id, .. }
            | Self::SourceStale { id, .. }
            | Self::Platform { id, .. } => id,
        }
    }

    /// One line naming the event, such as `run 0190… of orders_job failed`.
    #[must_use]
    pub fn summary(&self) -> String {
        match self {
            Self::Run {
                job,
                run_id,
                status,
                ..
            } => format!("run {run_id} of {job} ended {}", status.as_str()),
            Self::Node {
                node,
                run_id,
                event,
                ..
            } => format!("{node} {} in run {run_id}", node_event_word(*event)),
            Self::Check {
                check,
                run_id,
                status,
                ..
            } => format!(
                "{check} {} in run {run_id}",
                match status {
                    CheckEventKind::Failed => "failed",
                    CheckEventKind::Warn => "warned",
                }
            ),
            Self::SourceNewData { source, .. } => format!("{source} has new data"),
            Self::SourceStale {
                source,
                after_seconds,
                ..
            } => format!("{source} is older than {after_seconds} seconds"),
            Self::Platform { event, .. } => format!("platform event {}", event.as_str()),
        }
    }
}

const fn node_event_word(kind: NodeEventKind) -> &'static str {
    match kind {
        NodeEventKind::Success => "succeeded",
        NodeEventKind::Failed => "failed",
        NodeEventKind::Recovered => "recovered",
    }
}

/// A run a sensor's function asked for; the key defaults to the event ID and the job.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RequestedRun {
    pub job: String,
    pub run_key: String,
}

/// Storage order is authoritative even when a fact's occurrence time predates earlier pages.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SensorCursor {
    pub event_storage_id: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom: Option<String>,
}

/// What one call of a sensor's function returned; only the fenced scheduler applies it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum SensorFunctionOutcome {
    /// The function returned `None`.
    Handled {
        cursor: Option<String>,
    },
    Skipped {
        cursor: Option<String>,
        reason: String,
    },
    Requested {
        cursor: Option<String>,
        runs: Vec<RequestedRun>,
    },
    /// The function raised: its traceback.
    Failed {
        error: String,
    },
    TimedOut,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SensorEvaluationRequest {
    pub evaluation_id: Uuid,
    pub trigger_id: Uuid,
    /// The sensor's name, which the location finds its declaration by.
    pub sensor: String,
    pub location_id: Uuid,
    pub target: String,
    pub snapshot_id: Uuid,
    pub definition: SensorDefinition,
    pub definition_fingerprint: String,
    pub cursor: SensorCursor,
    /// The event the function is called with; `None` for a polling sensor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event: Option<SensorEvent>,
    /// The cursor once the event is handled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor_after: Option<SensorCursor>,
    pub epoch: i64,
    pub requested_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "phase", rename_all = "snake_case", deny_unknown_fields)]
pub enum SensorEvaluationOutcome {
    Requested,
    /// The function returned: runs it started, requests the job's `skip` overlap dropped, or why it skipped.
    Completed {
        cursor: SensorCursor,
        run_ids: Vec<Uuid>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        skipped: Vec<SkippedRequest>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        skip_reason: Option<String>,
    },
    /// The function raised, or asked for something the server refused; `error` explains it.
    Failed {
        code: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    TimedOut,
    Abandoned,
}

/// A run key the sensor matched but did not queue: the job's overlap is `skip` and `active_run_id` was active.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkippedRequest {
    pub run_key: String,
    pub active_run_id: Uuid,
}

/// The runs one evaluation queued and the requests it skipped, in request order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SensorRequests {
    pub run_ids: Vec<Uuid>,
    pub skipped: Vec<SkippedRequest>,
}

impl SensorEvaluationOutcome {
    pub(crate) const COMPLETED: &'static str = "completed";

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
    pub(crate) name: String,
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
    /// Present while the latest ticks fail.
    pub failing: Option<SensorFailing>,
}

/// The event position a condition reads from, and a polling sensor's own value after secret redaction; the value needs view.evidence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct PersonSensorCursor {
    pub event_storage_id: i64,
    pub custom: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct PersonSensorEvaluation {
    pub evaluation_id: Uuid,
    /// The latest phase: `requested` alone means the tick is in flight; otherwise `completed`, `failed`, `timed_out` or `abandoned`.
    pub phase: String,
    /// What the function did: `handled`, `skipped`, `started`, `failed`, `timed_out`, `abandoned`, or `running` while in flight.
    pub outcome: String,
    pub requested_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub cursor_before: PersonSensorCursor,
    /// Present when the tick completed.
    pub cursor_after: Option<PersonSensorCursor>,
    /// The event the function was called with; absent for a polling sensor.
    pub event_id: Option<String>,
    pub event_summary: Option<String>,
    /// The runs the tick started.
    pub run_ids: Vec<Uuid>,
    /// Why the function skipped, redacted; needs view.evidence.
    pub skip_reason: Option<String>,
    /// The failure code of a failed tick.
    pub failure_code: Option<String>,
    /// The traceback or refusal of a failed tick, redacted; needs view.evidence.
    pub error: Option<String>,
    /// For each requested run the job's `skip` overlap dropped, the run that was active.
    pub skipped_for: Vec<Uuid>,
    pub definition_fingerprint: String,
}

/// A sensor whose latest ticks failed: since when, and the event it is stuck on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct SensorFailing {
    /// When the first of the failed ticks since the last good one started.
    pub since: DateTime<Utc>,
    pub failed_ticks: usize,
    /// The event the failed ticks were called with; absent for a polling sensor.
    pub event_id: Option<String>,
    pub event_summary: Option<String>,
}

/// A run key is redacted and withheld like the custom cursor it usually derives from: empty without view.evidence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct PersonSensorRunRequest {
    pub run_key: String,
    pub run_id: Uuid,
    pub evaluation_id: Uuid,
    pub queued_at: DateTime<Utc>,
    pub status: crate::runs::models::RunStatus,
}

/// Which event `sqb sensor try` calls a sensor with, or `Recent` to list the ones it could.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(tag = "choice", rename_all = "snake_case", deny_unknown_fields)]
pub enum SensorEventChoice {
    Recent { limit: u32 },
    Latest,
    Event { event_id: String },
    Run { run_id: Uuid },
}

/// A published sensor and the events a trial chose for it, newest first.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct SensorTrialEvents {
    pub sensor: String,
    pub function: String,
    /// `None` for a polling sensor, which is called with no event.
    pub when: Option<SensorCondition>,
    pub timeout_seconds: u32,
    pub events: Vec<SensorEvent>,
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

/// Where a sensor reads events: its trigger, location, target and snapshot, and the UI origin run links start with.
#[derive(Clone, Copy, Debug)]
pub(crate) struct EventScope<'a> {
    pub(crate) trigger_id: Uuid,
    pub(crate) location_id: Uuid,
    pub(crate) target: &'a str,
    pub(crate) snapshot_id: Uuid,
    pub(crate) origin: Option<&'a str>,
}

/// Which way a condition reads its events: forward after a cursor position, or back from before one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Direction {
    After(i64),
    Before(i64),
}

/// The matching events one page held, at most the limit asked for, in reading order.
#[derive(Debug, Default)]
pub(crate) struct Scanned {
    pub(crate) events: Vec<SensorEvent>,
    /// The position of the last row read: the last match when the limit stopped the read.
    pub(crate) last: Option<i64>,
    /// Whether the page was full, so more may follow.
    pub(crate) full: bool,
}

/// The oldest event a sensor has not handled, or how far its cursor may move without one.
#[derive(Debug)]
pub(crate) enum NextEvent {
    Found {
        event: SensorEvent,
        cursor_after: SensorCursor,
    },
    /// No matching event: the cursor past what was read, and whether more remains to read now.
    Nothing { cursor: SensorCursor, more: bool },
}
