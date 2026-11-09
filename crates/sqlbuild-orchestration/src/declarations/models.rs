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
    /// Command settings; `vars` shows as `[REDACTED]` and text values pass through secret redaction.
    pub settings: BTreeMap<String, serde_json::Value>,
    /// A group's member jobs in run order; empty for a job.
    pub members: Vec<JobMember>,
    /// The groups this job belongs to.
    pub member_of: BTreeSet<String>,
    /// Selected nodes whose failure stops a group.
    pub critical_nodes: BTreeSet<String>,
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
