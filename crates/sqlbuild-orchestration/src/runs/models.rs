//! Frozen run requests and the run, node and claim records the control store keeps.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::credentials::models::Action;
use crate::declarations::models::PoolSlots;
use crate::runs::constants::RUN_LAUNCHED_EVENT;
use crate::runs::types::{CommandPlanner, PreparedPublication};
use crate::snapshots::models::WriterNode;

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

/// The `sqb` command a run executes; job and group runs always build.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RunCommand {
    #[default]
    Build,
    Test,
    Seed,
    Audit,
    Freshness,
}

impl RunCommand {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Build => "build",
            Self::Test => "test",
            Self::Seed => "seed",
            Self::Audit => "audit",
            Self::Freshness => "freshness",
        }
    }

    /// The command a planned verb runs, when workers run it.
    #[must_use]
    pub fn of_verb(verb: &str) -> Option<Self> {
        [
            Self::Build,
            Self::Test,
            Self::Seed,
            Self::Audit,
            Self::Freshness,
        ]
        .into_iter()
        .find(|command| command.as_str() == verb)
    }

    /// Build is the default, so job requests serialize as they always have.
    #[must_use]
    pub fn is_build(&self) -> bool {
        *self == Self::Build
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
    /// A group member the group did not run because a job it depends on failed.
    Skipped,
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
    #[serde(default, skip_serializing_if = "RunCommand::is_build")]
    pub command: RunCommand,
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
    /// Each pool the run takes, its amount and the pool's limit frozen with it; the server's cap may lower the limit.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub pools: BTreeMap<String, PoolSlots>,
    /// The selectors a person narrowed a job run with; reruns copy it with the rest of the request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub narrowing: Option<RunNarrowing>,
    /// Set on an automatic retry of a failed run, which is not claimed before `not_before`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry: Option<RunRetry>,
    /// Which attempt of its member job a group child is, from the second on; the first leaves it unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member_attempt: Option<u8>,
}

/// Which retry of the job's `[retry]` policy a run is, and when it may start.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RunRetry {
    pub attempt: u8,
    pub not_before: DateTime<Utc>,
}

impl RunRequest {
    /// Whether a queued run may start at `now`: only a retry still waiting out its delay may not.
    #[must_use]
    pub fn due(&self, now: DateTime<Utc>) -> bool {
        self.retry.is_none_or(|retry| retry.not_before <= now)
    }
}

/// A trusted operator names a published job; execution inputs are never supplied by the caller.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct JobRunRequest {
    /// A new random (UUID v4) or time-ordered (UUID v7) ID; name-based (v5) IDs are refused as the server derives those.
    pub run_id: Uuid,
    pub location_id: Uuid,
    pub target: String,
    pub job_key: String,
    pub reason: String,
    pub priority: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<RunWindow>,
    /// Rebuild every selected model from scratch; it rewrites history, as a window does.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub full_refresh: bool,
    /// Narrow the job with `sqb build` selectors; only nodes already in the job can remain.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub select: Vec<String>,
    /// Leave nodes out of the job with `sqb build` selectors.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclude: Vec<String>,
}

/// A retry derives its execution scope from a terminal run, never from caller selectors.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RerunRequest {
    /// A new random (UUID v4) or time-ordered (UUID v7) ID; name-based (v5) IDs are refused as the server derives those.
    pub run_id: Uuid,
    pub source_run_id: Uuid,
    pub from_failure: bool,
    pub reason: String,
    pub priority: i64,
    /// Rerun only these nodes of the source run; each must be in its selection.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nodes: Vec<String>,
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
    /// The published version the run executes, read from its snapshot.
    #[serde(default)]
    pub snapshot_generation: Option<i64>,
    #[serde(default)]
    pub git_revision: Option<String>,
    #[serde(default)]
    pub git_dirty: Option<bool>,
    /// The `sqb` version that published the snapshot.
    #[serde(default)]
    pub sqb_version: Option<String>,
    /// The deployment image the run is pinned to, when its location publishes one.
    #[serde(default)]
    pub image_id: Option<String>,
    pub job_key: Option<String>,
    pub kind: RunKind,
    pub origin: RunOrigin,
    pub parent_run_id: Option<Uuid>,
    pub root_run_id: Uuid,
    pub trigger_id: Option<Uuid>,
    pub trigger_name: Option<String>,
    pub tick_id: Option<Uuid>,
    pub rerun_of_run_id: Option<Uuid>,
    /// The stored actor: `principal:<id>` for a person, otherwise a system actor such as `system:scheduler`.
    pub requested_by: String,
    /// The display name of the requesting principal; never set for system actors.
    pub requester_name: Option<String>,
    pub priority: i64,
    pub rewrite_history: bool,
    pub status: RunStatus,
    pub status_reason: Option<String>,
    pub queued_at: DateTime<Utc>,
    pub launched_at: Option<DateTime<Utc>>,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub cancel_requested_at: Option<DateTime<Utc>>,
    pub selection_json: BTreeSet<String>,
    pub node_counts_json: BTreeMap<String, i64>,
    /// Error-severity audit failures the run let through; a group sums its members'.
    #[serde(default)]
    pub ignored_audit_failures: u32,
    pub last_event_storage_id: i64,
    /// How a person narrowed this run of a job, or the run it reruns, as typed; `None` when it runs the whole job.
    #[serde(
        default,
        deserialize_with = "crate::runs::_helpers::lifecycle::narrowing::frozen_narrowing"
    )]
    pub narrowing: Option<RunNarrowing>,
    /// The cursor window a backfill processes; `None` for every other run.
    #[serde(
        default,
        alias = "backfill_window",
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::runs::_helpers::lifecycle::narrowing::frozen_window"
    )]
    pub window: Option<RunWindow>,
    /// Set only when listing the queue: where the run stands and what it waits on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queue: Option<RunQueueState>,
    /// Which attempt of its member job a group child is, once the group retried it; absent for a first attempt and for every other run.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::runs::_helpers::lifecycle::narrowing::frozen_member_attempt"
    )]
    pub member_attempt: Option<u8>,
}

/// A queued run's place in claim order within its location and target, and what holds it back.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct RunQueueState {
    /// 1-based, in the order the scheduler claims: priority, then queue time.
    pub position: u32,
    /// Empty when nothing holds the run back and it starts on a free worker.
    pub waits: Vec<QueueWait>,
}

/// Why the scheduler cannot claim a queued run yet. Runs outside the listed location and target are counted, never named.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum QueueWait {
    /// Its group run has not started yet, or is stopping.
    GroupParent { run_id: Uuid },
    /// A job this one runs after in the same group has not finished.
    UpstreamJob {
        run_id: Uuid,
        job_key: String,
        /// The upstream job's attempt when the group is retrying it; absent for its first attempt.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        attempt: Option<u8>,
    },
    /// Another run of the same job is running.
    JobRunning { run_id: Uuid },
    /// Another run holds write claims on nodes this run writes.
    WriteClaim {
        run_id: Option<Uuid>,
        node_key: String,
        other_nodes: u32,
    },
    /// Running runs of the same location and target hold the slots of a pool this run takes.
    PoolFull { pool: String, holders: Vec<Uuid> },
    /// An earlier queued run takes the same pool, so it goes first.
    EarlierRun { run_id: Option<Uuid>, pool: String },
    /// An automatic retry waits out the job's retry delay.
    RetryDelay { attempt: u8, due_at: DateTime<Utc> },
}

/// The `sqb build` selectors one run of a job was narrowed with.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct RunNarrowing {
    pub select: Vec<String>,
    pub exclude: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum NodeOutcome {
    Success,
    Warn,
    Failed,
    Skipped,
    Cancelled,
    /// The run stopped before it reached the node: nothing upstream failed or skipped it.
    NotRun,
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
            Self::NotRun => "not_run",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct RunNodeRecord {
    pub node_key: String,
    pub status: String,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    /// Rows a statement inserted, merged or deleted, as the warehouse reported them.
    pub rows_affected: Option<i64>,
    /// The rows a fully rebuilt table holds, read from the warehouse's metadata where that count is exact.
    #[serde(default)]
    pub table_rows: Option<i64>,
    /// Rows written so far while the node runs, from microbatch batches or a loader's progress.
    #[serde(default)]
    pub rows_so_far: Option<i64>,
    /// Why a node that ran changed nothing, such as `unchanged since its last build`.
    #[serde(default)]
    pub no_change_reason: Option<String>,
    /// The node's materialization in the run's snapshot, such as `table`, `incremental` or `view`.
    #[serde(default)]
    pub materialization: Option<String>,
    pub error_code: Option<String>,
    pub skipped_because_of: Option<String>,
}

/// Recall event metadata of a run. Payloads are left out: they can carry SQL, request settings and error text.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct RunEventRecord {
    pub storage_id: i64,
    pub event_id: String,
    pub invocation_id: String,
    pub occurred_at: String,
    pub event_type: String,
    pub severity: Option<String>,
    pub producer: Option<String>,
    pub resource_id: Option<String>,
    pub resource_kind: Option<String>,
    pub resource_name: Option<String>,
    pub resource_attempt_id: Option<String>,
    pub statement_id: Option<String>,
    pub error_type: Option<String>,
    pub error_code: Option<String>,
    pub duration_ms: Option<f64>,
    pub outcome: Option<String>,
    pub row_count: Option<i64>,
    pub affected_rows: Option<i64>,
    /// A summary the scheduler writes for a group run's member events, such as a member retry; never error or SQL text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// One error-severity audit failure a run or one of its current members let through; never its SQL or evidence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct RunIgnoredAuditFailure {
    /// The run that evaluated the audit: the run itself or one of its group members.
    pub run_id: Uuid,
    pub job_key: Option<String>,
    pub occurred_at: String,
    /// The check's identity, as the checks endpoints name it.
    pub binding_key: Option<String>,
    pub audit_name: String,
    pub attached_target_kind: Option<String>,
    pub attached_target_name: Option<String>,
    pub attached_column_name: Option<String>,
    pub severity: String,
    pub violation_count: Option<i64>,
}

/// Warehouse statement metadata of a run: kind, timing, counts and outcome, never the SQL text.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct RunStatementRecord {
    pub invocation_id: String,
    pub statement_id: String,
    pub resource_id: Option<String>,
    pub resource_name: Option<String>,
    pub resource_kind: Option<String>,
    pub intent: Option<String>,
    pub statement_kind: Option<String>,
    pub purpose: Option<String>,
    pub query_id: Option<String>,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub outcome: Option<String>,
    pub error_type: Option<String>,
    pub error_code: Option<String>,
    pub duration_ms: Option<f64>,
    pub row_count: Option<i64>,
    pub affected_rows: Option<i64>,
    pub sql_available: bool,
    pub sql_unavailable_reason: Option<String>,
}

/// One statement's executed SQL; `sql` is null when capture was off or retention dropped it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct RunStatementSql {
    pub statement_id: String,
    pub sql: Option<String>,
}

/// Captured output and error text of a run's invocations; both can quote warehouse data.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct RunOutput {
    /// The newest captured lines per invocation, oldest first.
    pub lines: Vec<RunOutputLine>,
    /// Invocation failures and error diagnostics, oldest first.
    pub errors: Vec<RunErrorText>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct RunOutputLine {
    pub invocation_id: String,
    pub sequence: i64,
    pub stream: String,
    pub occurred_at: String,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct RunErrorText {
    pub invocation_id: String,
    pub occurred_at: Option<String>,
    pub error_type: Option<String>,
    pub message: String,
}

/// A node's outcome error in one run: redacted, bounded text and the project file of the node, never its SQL.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct RunNodeFailure {
    pub node_key: String,
    pub error_code: Option<String>,
    /// Null when the worker reported none or the run's history has been pruned.
    pub message: Option<String>,
    /// Relative to the project root, with `/` separators; null when the run's snapshot is gone.
    pub path: Option<String>,
}

/// One build attempt of a node by a run in this target; the same metadata as the run's node list.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct NodeRunRecord {
    pub run_id: Uuid,
    pub job_key: Option<String>,
    pub queued_at: DateTime<Utc>,
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
    /// Only a group sets it, on a member it did not run because a job the member depends on failed.
    Skipped,
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
            Self::Skipped => "skipped",
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
    /// Rows a running node has written so far; a worker sends at most one every few seconds per node.
    NodeProgress {
        node_key: String,
        rows_so_far: i64,
    },
    NodeFinished {
        node_key: String,
        outcome: NodeOutcome,
        rows_affected: Option<i64>,
        /// The rows a fully rebuilt table holds, where the warehouse's metadata count is exact.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        table_rows: Option<i64>,
        /// Why a node that ran changed nothing.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        no_change_reason: Option<String>,
        error_code: Option<String>,
        skipped_because_of: Option<String>,
        /// Redacted and bounded on the way in, whoever sends it; absent from older workers.
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "crate::runs::_helpers::lifecycle::error_text::bounded_error_text"
        )]
        error_message: Option<String>,
    },
    /// Observational attached-check facts never expand the frozen execution selection.
    CheckStarted {
        node_key: String,
    },
    CheckFinished {
        node_key: String,
        outcome: NodeOutcome,
    },
    /// A run with variables compiles them on the worker; this records each run node's resulting hash.
    Compiled {
        node_hashes: BTreeMap<String, String>,
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
    /// How many error-severity audit failures a build let through; sent just before `Finished`.
    AuditFailuresIgnored {
        count: u32,
    },
}

impl RunChange {
    pub(crate) const fn event_type(&self) -> &'static str {
        match self {
            Self::Launched { .. } => RUN_LAUNCHED_EVENT,
            Self::LaunchFailed { .. } => "run_launch_failed",
            Self::Started => "run_started",
            Self::NodeStarted { .. } => "run_node_started",
            Self::NodeProgress { .. } => "run_node_progress",
            Self::NodeFinished { .. } => "run_node_finished",
            Self::CheckStarted { .. } => "run_check_started",
            Self::CheckFinished { .. } => "run_check_finished",
            Self::Compiled { .. } => "run_compiled",
            Self::Unresponsive => "run_unresponsive",
            Self::Recovered => "run_recovered",
            Self::CancelRequested { .. } => "run_cancel_requested",
            Self::Finished { .. } => "run_terminated",
            Self::AuditFailuresIgnored { .. } => "run_audit_failures_ignored",
        }
    }

    /// The node or check the change is about, keyed `<kind>:<name>`.
    pub(crate) fn node_key(&self) -> Option<&str> {
        match self {
            Self::NodeStarted { node_key }
            | Self::NodeProgress { node_key, .. }
            | Self::NodeFinished { node_key, .. }
            | Self::CheckStarted { node_key }
            | Self::CheckFinished { node_key, .. } => Some(node_key),
            _ => None,
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

/// A running run's hold on a write-claimed node or a concurrency slot, with where the run is.
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct QueueHolder {
    pub(crate) held: String,
    pub(crate) run_id: Uuid,
    pub(crate) location_id: Uuid,
    pub(crate) target: String,
}

/// A run of a job that holds the job while it runs.
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct RunningJob {
    pub(crate) run_id: Uuid,
    pub(crate) job_key: String,
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
    #[serde(default)]
    pub(crate) ignored_audit_failures: i64,
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

/// A person's command as argument tokens after `sqb`, never shell text; selectors resolve on the server.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AdhocRunRequest {
    /// A new random (UUID v4) or time-ordered (UUID v7) ID; name-based (v5) IDs are refused as the server derives those.
    pub run_id: Uuid,
    pub location_id: Uuid,
    pub target: String,
    pub command: Vec<String>,
    pub reason: String,
    pub priority: i64,
}

/// A dry run of a command; nothing is queued or audited.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AdhocCheckRequest {
    pub location_id: Uuid,
    pub target: String,
    pub command: Vec<String>,
    pub priority: i64,
}

/// One SQL resource or Python node selectors resolve over: its `kind:name` key, upstream keys, tags, file folder and a source's loader.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlannerResource {
    pub key: String,
    pub deps: Vec<String>,
    pub tags: Vec<String>,
    pub folder: Option<String>,
    pub loader: Option<String>,
}

/// A parsed command: its verb, the exact node keys it selects and the flags it sets, by parser destination.
#[derive(Clone, Debug, PartialEq)]
pub struct CommandPlan {
    pub verb: String,
    pub selection: BTreeSet<String>,
    pub settings: BTreeMap<String, serde_json::Value>,
}

/// One permission the run needs and whether the person holds it on every resolved node.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AdhocPermission {
    pub action: Action,
    pub allowed: bool,
    /// Why it is refused, as the launch would report it.
    pub reason: Option<String>,
}

/// What launching the command would run and whether this person may launch it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AdhocCheck {
    pub snapshot_id: Option<Uuid>,
    pub verb: Option<String>,
    pub selection: Vec<String>,
    pub write_set: Vec<String>,
    pub settings: BTreeMap<String, serde_json::Value>,
    pub permissions: Vec<AdhocPermission>,
    /// Why the command cannot run regardless of permissions, such as a refused flag or an unknown selector.
    pub error: Option<String>,
    pub launchable: bool,
}

/// A person's change to the run queue: a published job, a command the planner resolves on the server, or a cancellation.
pub enum PersonRunRequest<'a> {
    Job(&'a dyn CommandPlanner, &'a JobRunRequest),
    Adhoc(&'a dyn CommandPlanner, &'a AdhocRunRequest),
    Cancel(&'a CancelRunRequest),
    /// A build command planned on the location for `(location_id, target)`; it never becomes a run.
    Plan(
        &'a dyn CommandPlanner,
        (Uuid, &'a str, &'a crate::platform::models::PlanRequest),
    ),
}

/// Installation configuration: how long a finished run keeps its worker log, by age and by the number of newer finished runs in its location target, whichever comes first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RunLogRetention {
    days: u16,
    count: u16,
}

impl RunLogRetention {
    pub fn new(days: u16, count: u16) -> Result<Self, crate::store::errors::ControlError> {
        if days == 0 || count == 0 {
            return Err(crate::store::errors::ControlError::Invalid(
                "run log retention must keep at least one day and one run".into(),
            ));
        }
        Ok(Self { days, count })
    }

    pub(crate) fn cutoff(self, now: DateTime<Utc>) -> DateTime<Utc> {
        now - chrono::TimeDelta::days(i64::from(self.days))
    }

    #[must_use]
    pub const fn days(self) -> u16 {
        self.days
    }

    #[must_use]
    pub const fn count(self) -> u16 {
        self.count
    }
}

impl Default for RunLogRetention {
    fn default() -> Self {
        Self {
            days: crate::runs::constants::DEFAULT_RUN_LOG_RETENTION_DAYS,
            count: crate::runs::constants::DEFAULT_RUN_LOG_RETENTION_COUNT,
        }
    }
}

/// Prepared publications by snapshot ID; snapshots never change, so an entry stays valid until evicted.
#[derive(Debug)]
pub struct PublicationCache {
    pub(crate) slots: std::sync::Mutex<SnapshotSlots>,
}

impl PublicationCache {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            slots: std::sync::Mutex::new(SnapshotSlots::new()),
        }
    }
}

impl Default for PublicationCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Publications prepared once per snapshot, least recently used first out within each location and target.
#[derive(Debug)]
pub(crate) struct SnapshotSlots {
    /// Counts lookups, so a larger `used` is more recent.
    pub(crate) clock: u64,
    pub(crate) slots: Vec<SnapshotSlot>,
}

impl SnapshotSlots {
    pub(crate) const fn new() -> Self {
        Self {
            clock: 0,
            slots: Vec::new(),
        }
    }
}

/// One snapshot's publication, prepared by the first request that needs it while later ones wait.
#[derive(Debug)]
pub(crate) struct SnapshotSlot {
    pub(crate) scope: (Uuid, String),
    pub(crate) snapshot_id: Uuid,
    pub(crate) used: u64,
    pub(crate) cell: std::sync::Arc<tokio::sync::OnceCell<std::sync::Arc<CachedPublication>>>,
}

/// What every plan of one publication shares: the planner's index, each node's write-set facts and every node key.
#[derive(Debug)]
pub(crate) struct CachedPublication {
    pub(crate) prepared: std::sync::Arc<dyn PreparedPublication>,
    pub(crate) writers: BTreeMap<String, WriterNode>,
    pub(crate) node_keys: BTreeSet<String>,
}

/// A selection to evaluate, as typed: `select` and `exclude` each hold space-separated selectors, as `sqb build -s … --exclude …` takes them.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectionQuery {
    #[serde(default)]
    pub select: String,
    #[serde(default)]
    pub exclude: String,
    /// How many sorted keys to skip.
    pub offset: Option<u32>,
    /// Page size; defaults to 1,000, clamped to 1..10,000.
    pub limit: Option<u32>,
}

/// The nodes `sqb build --select … --exclude …` selects in the current publication, or why it refuses the selectors.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct SelectionEvaluation {
    /// The publication evaluated; `None` before the first one.
    pub snapshot_id: Option<Uuid>,
    /// Every selected node, across all pages.
    pub count: u64,
    /// One page of the selected `kind:name` keys, sorted.
    pub keys: Vec<String>,
    /// The offset of the next page, when there is one.
    pub next_offset: Option<u32>,
    pub error: Option<SelectionProblem>,
}

/// Why `sqb` refuses a selection, where, and what might have been meant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct SelectionProblem {
    /// The selector error code, such as `S007`; `None` when the selection as a whole is refused.
    pub code: Option<String>,
    pub message: String,
    /// `sqb`'s own help line, such as "did you mean 'orders'?".
    pub help: Option<String>,
    /// The selector token the error comes from, when one token causes it.
    pub location: Option<SelectionLocation>,
    /// Replacements for that whole token, closest first.
    pub suggestions: Vec<String>,
    /// The comma part of that token `sqb` refused, when it names one selector value.
    pub refused: Option<SelectionRefusal>,
}

/// One refused comma part: its exact text and position in the token, its selector type and value.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct SelectionRefusal {
    /// The part as typed, such as `+tag:finanse`.
    pub part: String,
    /// Which comma part of the token it is, from 0.
    pub index: u32,
    /// `tag`, `path`, `job`, `source`, `seed`, `kind`, `test`, `name`, `glob`, or a Python node type such as `asset`.
    #[serde(rename = "type")]
    pub selector_type: String,
    /// The value the part names, without its type prefix or `+` markers, such as `finanse`.
    pub value: String,
}

/// One selector token in the request's `select` or `exclude` text.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct SelectionLocation {
    pub field: SelectionField,
    pub token: String,
    /// Where the token starts and ends in its field, in UTF-16 code units, as JavaScript indexes strings.
    pub start: u32,
    pub end: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SelectionField {
    Select,
    Exclude,
}

/// The values each selector type takes in the current publication, with how many nodes each one selects.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct SelectionVocabulary {
    /// The publication described; `None` before the first one.
    pub snapshot_id: Option<Uuid>,
    /// `tag:` values: the SQL resources and Python nodes with each tag.
    pub tags: Vec<VocabularyEntry>,
    /// `path:` folders such as `models/staging`: the nodes below each, at any depth.
    pub paths: Vec<VocabularyEntry>,
    /// `job:` names: the nodes each job or job group selects.
    pub jobs: Vec<VocabularyEntry>,
    /// `source:` names, one node each.
    pub sources: Vec<VocabularyEntry>,
    /// `seed:` names, one node each.
    pub seeds: Vec<VocabularyEntry>,
    /// `kind:` values: the nodes of each kind.
    pub kinds: Vec<VocabularyEntry>,
}

/// One selector value and the number of nodes it selects.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct VocabularyEntry {
    pub value: String,
    pub count: u64,
}
