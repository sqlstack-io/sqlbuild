//! Resolved, serializable declarations; Python objects never enter scheduler state.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::sensors::models::SensorDefinition;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TriggerStatus {
    #[default]
    Running,
    Paused,
}

impl TriggerStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Paused => "paused",
        }
    }
}

/// Selections have already been resolved by the code location against the compiled snapshot.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobDefinition {
    pub description: String,
    pub selection: BTreeSet<String>,
    #[serde(default)]
    pub settings: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub concurrency_groups: BTreeSet<String>,
    /// The selectors as written, for display; `selection` is what runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub select: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exclude: Option<String>,
    /// The targets the declaration is limited to; empty for every target.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub targets: BTreeSet<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declared_at: Option<DeclaredAt>,
    /// Who answers for the job, as written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    #[serde(default, skip_serializing_if = "Overlap::is_queue")]
    pub overlap: Overlap,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_failure: Option<OnFailure>,
}

/// What a schedule tick or sensor request does while a run of the same job is active; people's requests always queue.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Overlap {
    #[default]
    Queue,
    Skip,
}

impl Overlap {
    #[must_use]
    pub const fn is_queue(&self) -> bool {
        matches!(self, Self::Queue)
    }
}

/// A failed or partial run reruns from failure up to `retries` times, each `retry_delay_seconds` after the last ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct OnFailure {
    pub retries: u8,
    pub retry_delay_seconds: u32,
}

/// Where a declaration was written, relative to the project root.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct DeclaredAt {
    pub path: String,
    pub line: u32,
}

/// A virtual parent shares run controls, but only its physical children claim node writers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GroupDefinition {
    #[serde(flatten)]
    pub job: JobDefinition,
    pub jobs: BTreeMap<String, BTreeSet<String>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Schedule {
    Cron {
        expression: String,
        timezone: String,
    },
    Calendar {
        times: BTreeSet<DateTime<Utc>>,
        refresh_at: DateTime<Utc>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct TriggerDefinition {
    pub description: String,
    pub job_key: String,
    pub schedule: Schedule,
    #[serde(default)]
    pub starts_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub default_status: TriggerStatus,
    /// The targets the declaration is limited to; empty for every target.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub targets: BTreeSet<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declared_at: Option<DeclaredAt>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Definitions {
    pub jobs: BTreeMap<String, JobDefinition>,
    pub groups: BTreeMap<String, GroupDefinition>,
    pub triggers: BTreeMap<String, TriggerDefinition>,
    pub sensors: BTreeMap<String, SensorDefinition>,
    pub roles: Option<serde_json::Value>,
}

/// Legacy schedule documents retain their stored shape; sensors add a distinct condition shape.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(untagged)]
pub enum TriggerDeclaration {
    Schedule(TriggerDefinition),
    Sensor(SensorDefinition),
}

/// A job or group of the current published snapshot. Settings are left out of person reads.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct JobRecord {
    pub job_key: String,
    /// `job` or `group`; a group's selection is the union of its member jobs' selections.
    pub kind: crate::runs::models::RunKind,
    pub description: String,
    pub selection: BTreeSet<String>,
    pub concurrency_groups: BTreeSet<String>,
    pub snapshot_id: uuid::Uuid,
    /// A job's selectors as written; absent for a group and for snapshots published before they were recorded.
    pub select: Option<String>,
    pub declared_at: Option<DeclaredAt>,
    pub owner: Option<String>,
}

/// A `sqb_jobs` row as stored.
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct StoredJob {
    pub job_key: String,
    pub kind: crate::runs::models::RunKind,
    pub snapshot_id: uuid::Uuid,
    pub definition_json: JobDefinition,
}

/// One job or group as declared, for a person who may list the target; free-form setting values are withheld.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct JobDetailRecord {
    pub job_key: String,
    pub kind: crate::runs::models::RunKind,
    pub description: String,
    pub snapshot_id: uuid::Uuid,
    pub selection: BTreeSet<String>,
    pub concurrency_groups: BTreeSet<String>,
    /// Command settings; `vars` and a `defer_*` value that is not a target name show as `[REDACTED]`, and text values pass through secret redaction.
    pub settings: BTreeMap<String, serde_json::Value>,
    /// A group's member jobs in run order; empty for a job.
    pub members: Vec<JobMember>,
    /// The groups this job belongs to.
    pub member_of: BTreeSet<String>,
    /// Selected nodes whose failure stops a group.
    pub critical_nodes: BTreeSet<String>,
    pub select: Option<String>,
    pub exclude: Option<String>,
    /// Empty when the declaration applies to every target.
    pub targets: BTreeSet<String>,
    pub declared_at: Option<DeclaredAt>,
    /// The cursor type every selected cursor node shares; absent when none has a cursor or types differ.
    pub cursor_type: Option<String>,
    pub owner: Option<String>,
    pub overlap: Overlap,
    /// A member job's own policy applies only when it runs alone; inside a group run the group's applies.
    pub on_failure: Option<OnFailure>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct JobMember {
    pub job_key: String,
    /// Members this one waits for, from lineage and from the group's `after`.
    pub waits_for: BTreeSet<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct TriggerRecord {
    pub trigger_id: uuid::Uuid,
    pub name: String,
    pub job_key: String,
    pub definition_json: TriggerDeclaration,
    pub status: TriggerStatus,
    pub last_tick_for: Option<DateTime<Utc>>,
    pub next_due_at: Option<DateTime<Utc>>,
}

/// A trigger as people list it, with its newest tick so a list needs no read per trigger.
#[derive(Clone, Debug, PartialEq, Serialize, utoipa::ToSchema)]
pub struct PersonTriggerRecord {
    #[serde(flatten)]
    pub trigger: TriggerRecord,
    pub latest_tick: Option<crate::scheduler::models::TickRecord>,
    /// A running schedule's next due times from `next_due_at` on, at most five; empty otherwise.
    pub upcoming: Vec<DateTime<Utc>>,
}

/// One operator change of a trigger, as a person reads it: what changed, who changed it, when and why.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct PersonTriggerChange {
    /// The change's own ID, as recorded in the security audit.
    pub event_id: uuid::Uuid,
    pub change: TriggerChange,
    /// The audited actor, such as `principal:<id>` for a person.
    pub actor: String,
    /// The person's name when the actor is a person who still exists.
    pub actor_name: Option<String>,
    /// The operator's reason, with credential-shaped text redacted.
    pub reason: String,
    pub occurred_at: DateTime<Utc>,
    /// The publication that was current when the change was made.
    pub snapshot_id: uuid::Uuid,
}

/// Operator changes are immutable facts; clearing an override restores the published code default.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TriggerChange {
    Enable,
    Pause,
    ResetDefault,
    SetCursor { time: DateTime<Utc> },
    ResetCursor,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct TriggerChangeRequest {
    pub event_id: uuid::Uuid,
    pub trigger_id: uuid::Uuid,
    pub actor: String,
    pub reason: String,
    pub change: TriggerChange,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct TriggerChangeRecord {
    pub request_json: TriggerChangeRequest,
    pub occurred_at: DateTime<Utc>,
    pub snapshot_id: uuid::Uuid,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct EditableTriggerState {
    pub(crate) definition_json: TriggerDeclaration,
    pub(crate) snapshot_id: uuid::Uuid,
    pub(crate) started_at: DateTime<Utc>,
    pub(crate) last_tick_for: Option<DateTime<Utc>>,
    pub(crate) status_override: Option<TriggerStatus>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct PreviousTriggerState {
    pub(crate) definition_json: TriggerDefinition,
    pub(crate) started_at: DateTime<Utc>,
    pub(crate) last_tick_for: Option<DateTime<Utc>>,
    pub(crate) status_override: Option<TriggerStatus>,
}

/// Each member job's waits, and why it waits: an explicit `after`, or the first lineage edge found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct GroupWaits {
    pub(crate) edges: BTreeMap<String, BTreeSet<String>>,
    pub(crate) reasons: BTreeMap<(String, String), String>,
}
