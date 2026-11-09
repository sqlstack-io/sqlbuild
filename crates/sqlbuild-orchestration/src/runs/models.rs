//! Frozen run requests and the run, node and claim records the control store keeps.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::credentials::models::Action;
use crate::runs::types::CommandPlanner;

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

/// A person's command as argument tokens after `sqb`, never shell text; selectors resolve on the server.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AdhocRunRequest {
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

/// One graph resource selectors resolve over: its `kind:name` key, upstream keys, tags and model folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlannerResource {
    pub key: String,
    pub deps: Vec<String>,
    pub tags: Vec<String>,
    pub folder: Option<String>,
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
}
