//! Frozen run requests and the run, node and claim records the control store keeps.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RunKind {
    Job,
    Group,
    Adhoc,
}

impl RunKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Job => "job",
            Self::Group => "group",
            Self::Adhoc => "adhoc",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RunOrigin {
    Manual,
    Schedule,
    Sensor,
    Rerun,
    Backfill,
}

impl RunOrigin {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Schedule => "schedule",
            Self::Sensor => "sensor",
            Self::Rerun => "rerun",
            Self::Backfill => "backfill",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Queued,
    Launched,
    Running,
    Unresponsive,
    Success,
    Warn,
    Partial,
    Failed,
    Cancelled,
}

impl RunStatus {
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        !matches!(
            self,
            Self::Queued | Self::Launched | Self::Running | Self::Unresponsive
        )
    }
}

/// Inclusive typed CLI cursor bounds; the native planner derives exclusive execution ends.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RunWindow {
    Timestamp {
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    },
    Integer {
        start: i64,
        end: i64,
    },
}

/// The authorized selection and pinned execution inputs are immutable once requested.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct RunRequest {
    pub run_id: Uuid,
    pub location_id: Uuid,
    pub target: String,
    pub snapshot_id: Uuid,
    pub image_id: Option<String>,
    pub kind: RunKind,
    pub origin: RunOrigin,
    pub job_key: Option<String>,
    pub parent_run_id: Option<Uuid>,
    pub root_run_id: Uuid,
    pub trigger_id: Option<Uuid>,
    pub tick_id: Option<Uuid>,
    pub rerun_of_run_id: Option<Uuid>,
    pub requested_by: String,
    pub priority: i64,
    pub selection: BTreeSet<String>,
    pub write_set: BTreeSet<String>,
    pub settings: BTreeMap<String, serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<RunWindow>,
    pub concurrency_groups: BTreeSet<String>,
}

/// A trusted operator names a published job; execution inputs are never supplied by the caller.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct JobRunRequest {
    pub run_id: Uuid,
    pub location_id: Uuid,
    pub target: String,
    pub job_key: String,
    pub reason: String,
    pub priority: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<RunWindow>,
}

/// A retry derives its execution scope from a terminal run, never from caller selectors.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RerunRequest {
    pub run_id: Uuid,
    pub source_run_id: Uuid,
    pub from_failure: bool,
    pub reason: String,
    pub priority: i64,
}

/// Browser cancellation cannot supply an actor, claim or lifecycle transition.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CancelRunRequest {
    pub run_id: Uuid,
    pub event_id: Uuid,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct RunRecord {
    pub run_id: Uuid,
    pub location_id: Uuid,
    pub target: String,
    pub snapshot_id: Uuid,
    pub job_key: Option<String>,
    pub status: RunStatus,
    pub queued_at: DateTime<Utc>,
    pub launched_at: Option<DateTime<Utc>>,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub selection_json: BTreeSet<String>,
    pub node_counts_json: BTreeMap<String, i64>,
    pub last_event_storage_id: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum NodeOutcome {
    Success,
    Warn,
    Failed,
    Skipped,
    Cancelled,
}

impl NodeOutcome {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Warn => "warn",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct RunNodeRecord {
    pub node_key: String,
    pub status: String,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub rows_affected: Option<i64>,
    pub error_code: Option<String>,
    pub skipped_because_of: Option<String>,
}

/// The database owns the claim before a launcher sees the pinned request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct RunClaim {
    pub claim_id: Uuid,
    pub request: RunRequest,
}

/// Workers compile locally and compare against this immutable snapshot, not the current deploy.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct RunExecution {
    pub claim: RunClaim,
    pub environment_hash: String,
    pub sqb_version: String,
    pub node_hashes: BTreeMap<String, String>,
    /// Derived from completed sibling facts and pinned lineage, never a mutable request rewrite.
    #[serde(default)]
    pub skipped_nodes: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ExecutionMetadata {
    pub(crate) request_json: RunRequest,
    pub(crate) environment_hash: String,
    pub(crate) sqb_version: String,
}

/// Terminal outcomes cannot be confused with a request to change an active status.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RunOutcome {
    Success,
    Warn,
    Partial,
    Failed,
    Cancelled,
}

impl RunOutcome {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Warn => "warn",
            Self::Partial => "partial",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

/// This is a server-owned instruction, not an arbitrary status supplied by a worker.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RunChange {
    Launched {
        container_id: Option<String>,
    },
    /// A failed launch response is not physical evidence that the worker has stopped.
    LaunchFailed {
        code: String,
    },
    Started,
    NodeStarted {
        node_key: String,
    },
    NodeFinished {
        node_key: String,
        outcome: NodeOutcome,
        rows_affected: Option<i64>,
        error_code: Option<String>,
        skipped_because_of: Option<String>,
    },
    /// Observational attached-check facts never expand the frozen execution selection.
    CheckStarted {
        node_key: String,
    },
    CheckFinished {
        node_key: String,
        outcome: NodeOutcome,
    },
    Unresponsive,
    Recovered,
    CancelRequested {
        actor: String,
        #[serde(default)]
        reason: String,
    },
    Finished {
        outcome: RunOutcome,
        reason: String,
        exit_code: Option<i64>,
    },
}

impl RunChange {
    pub(crate) const fn event_type(&self) -> &'static str {
        match self {
            Self::Launched { .. } => "run_launched",
            Self::LaunchFailed { .. } => "run_launch_failed",
            Self::Started => "run_started",
            Self::NodeStarted { .. } => "run_node_started",
            Self::NodeFinished { .. } => "run_node_finished",
            Self::CheckStarted { .. } => "run_check_started",
            Self::CheckFinished { .. } => "run_check_finished",
            Self::Unresponsive => "run_unresponsive",
            Self::Recovered => "run_recovered",
            Self::CancelRequested { .. } => "run_cancel_requested",
            Self::Finished { .. } => "run_terminated",
        }
    }

    pub(crate) fn finished_node(&self) -> Option<(&str, NodeOutcome)> {
        match self {
            Self::NodeFinished {
                node_key, outcome, ..
            }
            | Self::CheckFinished { node_key, outcome } => Some((node_key, *outcome)),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RunChangeRequest {
    pub event_id: Uuid,
    pub run_id: Uuid,
    pub claim_id: Option<Uuid>,
    pub change: RunChange,
}

/// Constructed after authentication; this authority is never deserialized from a request body.
#[derive(Clone, Copy, Debug)]
pub enum RunAuthority<'a> {
    Scheduler(&'a crate::scheduler::models::SchedulerLease),
    Worker { run_id: Uuid, claim_id: Uuid },
    Operator(&'a crate::credentials::models::AuditContext),
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct QueueCandidate {
    pub(crate) request_json: RunRequest,
    pub(crate) queued_at: DateTime<Utc>,
}

/// Fairness covers runnable launcher scopes, not queues the installation cannot dispatch.
#[derive(Clone, Copy, Debug)]
pub struct RunDispatch<'a> {
    pub scope: Option<(Uuid, &'a str)>,
    pub eligible: Option<&'a BTreeSet<(Uuid, String)>>,
}

impl<'a> From<Option<(Uuid, &'a str)>> for RunDispatch<'a> {
    fn from(scope: Option<(Uuid, &'a str)>) -> Self {
        Self {
            scope,
            eligible: None,
        }
    }
}

impl<'a> From<(Uuid, &'a str)> for RunDispatch<'a> {
    fn from(scope: (Uuid, &'a str)) -> Self {
        Some(scope).into()
    }
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct ChangingRun {
    pub(crate) status: RunStatus,
    pub(crate) worker_instance: Option<Uuid>,
    pub(crate) cancel_requested_at: Option<DateTime<Utc>>,
    pub(crate) node_counts_json: BTreeMap<String, i64>,
}

/// Virtual parents and physical children use the same lifecycle evidence during group progress.
#[derive(Debug, Deserialize)]
pub(crate) struct GroupRun {
    pub(crate) request_json: RunRequest,
    pub(crate) queued_at: DateTime<Utc>,
    #[serde(flatten)]
    pub(crate) state: ChangingRun,
}

/// A queued top-level run and the limits it takes when admitted, in queue order.
#[derive(Debug)]
pub(crate) struct LimitWaiter {
    pub(crate) rank: (std::cmp::Reverse<i64>, DateTime<Utc>, Uuid),
    pub(crate) run_id: Uuid,
    pub(crate) limits: BTreeSet<String>,
}

#[derive(Debug)]
pub(crate) struct GroupContext {
    pub(crate) parent: GroupRun,
    pub(crate) children: Vec<GroupRun>,
    pub(crate) definitions: crate::declarations::models::Definitions,
    pub(crate) critical_failed: bool,
}

/// Differing retries retain conflict evidence while preserving the first output bytes.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct OutputPublication {
    pub inserted: u32,
    pub duplicates: u32,
    pub conflicts: u32,
}

/// One invocation's records; the server owns its claim namespace, start time and version.
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RecallBatch {
    pub invocation_id: String,
    pub records: Vec<RecallRecord>,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RecallRecord {
    pub table: sqlbuild_recall::schema::models::Table,
    pub values: BTreeMap<String, sqlbuild_recall::records::models::RecallValue>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ClaimMetadata {
    pub(crate) claimed_at: DateTime<Utc>,
    pub(crate) sqb_version: String,
}

/// A physical launcher uses the database clock and current liveness, never an old run event.
#[derive(Debug, Deserialize)]
pub struct RunObservation {
    pub status: RunStatus,
    pub cancel_requested_at: Option<DateTime<Utc>>,
    pub claimed_at: Option<DateTime<Utc>>,
    pub last_heartbeat_at: Option<DateTime<Utc>>,
    pub observed_at: DateTime<Utc>,
}
