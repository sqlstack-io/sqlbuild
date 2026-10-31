//! Frozen requests and rebuildable run projections; lifecycle authority remains in Recall.

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
    PresumedFailed,
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
    pub concurrency_groups: BTreeSet<String>,
    pub overlap: crate::scheduler::models::Overlap,
}

/// A trusted operator names a published job; execution inputs are never supplied by the caller.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobRunRequest {
    pub run_id: Uuid,
    pub location_id: Uuid,
    pub target: String,
    pub job_key: String,
    pub reason: String,
    pub priority: i64,
}

/// A retry derives its execution scope from a terminal run, never from caller selectors.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RerunRequest {
    pub run_id: Uuid,
    pub source_run_id: Uuid,
    pub from_failure: bool,
    pub reason: String,
    pub priority: i64,
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
    pub attempt: i64,
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
    PresumedFailed,
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
            Self::PresumedFailed => "presumed_failed",
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
            Self::Started => "run_started",
            Self::NodeStarted { .. } => "run_node_started",
            Self::NodeFinished { .. } => "run_node_finished",
            Self::Unresponsive => "run_unresponsive",
            Self::Recovered => "run_recovered",
            Self::CancelRequested { .. } => "run_cancel_requested",
            Self::Finished { .. } => "run_terminated",
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

/// Parent overlap locks and installation-wide limits occupy different resource namespaces.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ConcurrencyResource {
    Limit(String),
    Overlap(Uuid, String, String),
}

/// A transaction-local safety calculation, never a second persisted allocation ledger.
#[derive(Debug)]
pub(crate) struct GroupCapacity {
    pub(crate) available: BTreeMap<ConcurrencyResource, i64>,
    pub(crate) held: BTreeMap<Uuid, BTreeSet<ConcurrencyResource>>,
    pub(crate) needed: BTreeMap<Uuid, BTreeSet<ConcurrencyResource>>,
    pub(crate) waiting: Vec<QueueCandidate>,
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
