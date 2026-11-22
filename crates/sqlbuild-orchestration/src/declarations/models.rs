//! Resolved, serializable declarations; Python objects never enter scheduler state.

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU32;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
    /// Each pool the job takes, with its amount and the pool's limit on the target.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub pools: BTreeMap<String, PoolSlots>,
    /// The most of the job's nodes a run builds at once on the target; absent from older snapshots.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_concurrency: Option<ResolvedConcurrency>,
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
    /// The worker container's limits for this job; the launcher's `max_memory` caps memory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resources: Option<RunResources>,
}

/// Container limits a job asks for; each unset limit falls back to the launcher's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunResources {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<MemorySize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpus: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pids: Option<u64>,
}

/// Bytes, written as an integer or with a `k`, `m` or `g` suffix (powers of 1024), as Docker's `--memory` reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct MemorySize(pub u64);

impl MemorySize {
    /// `"512m"`, `"2g"` or `"1073741824"`; the size must be at least one byte.
    pub fn parse(text: &str) -> Result<Self, String> {
        let text = text.trim().to_ascii_lowercase();
        let (digits, scale) = match text.strip_suffix('b').unwrap_or(&text) {
            value if value.ends_with('k') => (&value[..value.len() - 1], 1u64 << 10),
            value if value.ends_with('m') => (&value[..value.len() - 1], 1u64 << 20),
            value if value.ends_with('g') => (&value[..value.len() - 1], 1u64 << 30),
            value => (value, 1),
        };
        match digits.parse::<u64>().map(|count| count.checked_mul(scale)) {
            Ok(Some(bytes)) if bytes > 0 => Ok(Self(bytes)),
            _ => Err(format!(
                "memory must be a positive size such as \"512m\" or \"2g\", not {text:?}"
            )),
        }
    }
}

impl<'de> Deserialize<'de> for MemorySize {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Written {
            Bytes(u64),
            Text(String),
        }
        match Written::deserialize(deserializer)? {
            Written::Bytes(0) => Err(serde::de::Error::custom("memory must be at least one byte")),
            Written::Bytes(bytes) => Ok(Self(bytes)),
            Written::Text(text) => Self::parse(&text).map_err(serde::de::Error::custom),
        }
    }
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
    /// Each member's node-level reasons to wait for the members in `jobs`; members without one are left out.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub wait_reasons: BTreeMap<String, Vec<WaitReason>>,
    /// Each member whose `[dependencies] on_failure` is not `partial`, with its policy.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub dependency_failure: BTreeMap<String, DependencyFailure>,
}

impl GroupDefinition {
    /// What `member` does when a job it depends on failed.
    #[must_use]
    pub fn member_dependency_failure(&self, member: &str) -> DependencyFailure {
        self.dependency_failure
            .get(member)
            .copied()
            .unwrap_or_default()
    }
}

/// One reason a member waits: `from_node` of `upstream_job` feeds its `to_node`, directly or through nodes no member selects.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct WaitReason {
    pub upstream_job: String,
    pub from_node: String,
    pub to_node: String,
}

/// The most of a job's nodes a run of it builds at once on its target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ResolvedConcurrency {
    pub limit: u64,
}

/// How many slots of a pool a job takes, and the pool's limit on the job's target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PoolSlots {
    #[schema(value_type = u32)]
    pub amount: NonZeroU32,
    #[schema(value_type = u32)]
    pub limit: NonZeroU32,
}

/// A pool a job takes: its slots, the pool's limit, and the other jobs and groups taking it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct PoolUse {
    pub pool: String,
    /// The slots the job asks for.
    pub amount: u32,
    /// The project's limit on the target.
    pub limit: u32,
    /// This server's `pool_caps` entry for the location and target; absent when it sets none.
    pub cap: Option<u32>,
    /// The limit on this server: the project's limit, lowered to the cap.
    pub server_limit: u32,
    /// The job asks for more slots than `server_limit`, so on this server it takes the whole pool.
    pub clamped: bool,
    pub shared_with: BTreeSet<String>,
}

/// Each launcher's `pool_caps` on this server, by location and target.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ServerPoolCaps {
    pub launchers: BTreeMap<(Uuid, String), BTreeMap<String, NonZeroU32>>,
}

impl ServerPoolCaps {
    /// The caps of the launcher serving `target` of `location_id`; none when no launcher here serves it.
    #[must_use]
    pub fn on(&self, (location_id, target): (Uuid, &str)) -> BTreeMap<String, NonZeroU32> {
        self.launchers
            .get(&(location_id, target.to_owned()))
            .cloned()
            .unwrap_or_default()
    }
}

/// What a group member does when a job it depends on failed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DependencyFailure {
    /// Build what can still be built, skipping nodes downstream of the failed ones.
    #[default]
    Partial,
    /// Do not run the member: its nodes end skipped.
    Skip,
    /// Run every node regardless.
    Run,
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
    /// Each pool the job takes, by pool name.
    pub pools: Vec<PoolUse>,
    /// The job's own `concurrency` setting, models building at once in a run; absent uses the project's.
    pub concurrency: Option<u64>,
    /// The job's own `ignore_audit_failures` setting; absent uses the target's, then the project's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ignore_audit_failures: Option<bool>,
    /// The most of the job's nodes a run builds at once on the snapshot's target; absent for older snapshots.
    pub resolved_concurrency: Option<ResolvedConcurrency>,
    pub snapshot_id: uuid::Uuid,
    /// A job's selectors as written; absent for a group and for snapshots published before they were recorded.
    pub select: Option<String>,
    pub declared_at: Option<DeclaredAt>,
    pub owner: Option<String>,
    pub overlap: Overlap,
    pub on_failure: Option<OnFailure>,
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
    /// Each pool the job takes, by pool name.
    pub pools: Vec<PoolUse>,
    /// The job's own `concurrency` setting, models building at once in a run; absent uses the project's.
    pub concurrency: Option<u64>,
    /// The job's own `ignore_audit_failures` setting; absent uses the target's, then the project's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ignore_audit_failures: Option<bool>,
    /// The most of the job's nodes a run builds at once on the snapshot's target; absent for older snapshots.
    pub resolved_concurrency: Option<ResolvedConcurrency>,
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
    /// Also applies to the job inside a group run: the group retries it while the members that depend on it wait.
    pub on_failure: Option<OnFailure>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct JobMember {
    pub job_key: String,
    /// Members this one waits for, from the lineage between their nodes.
    pub waits_for: BTreeSet<String>,
    /// Node by node, why it waits for each of `waits_for`; a wait with none follows lineage through another member.
    pub reasons: Vec<WaitReason>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct TriggerRecord {
    pub trigger_id: uuid::Uuid,
    pub name: String,
    /// The job or group a schedule starts; `None` for a sensor, whose function chooses.
    pub job_key: Option<String>,
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
    /// A sensor whose latest ticks failed: since when and the event it is stuck on.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sensor_failing: Option<crate::sensors::models::SensorFailing>,
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
    SetCursor {
        time: DateTime<Utc>,
    },
    /// A sensor reads every retained event again from the start.
    ResetCursor,
    /// A sensor's stuck `event` counts as delivered without calling the function again.
    SkipEvent {
        event: String,
    },
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

/// A schedule's state before a republish: its timing columns, which always read, and its definition, `None` when an earlier release stored it in a form that no longer decodes.
#[derive(Clone, Debug)]
pub(crate) struct PreviousTriggerState {
    pub(crate) definition_json: Option<TriggerDefinition>,
    pub(crate) started_at: DateTime<Utc>,
    pub(crate) last_tick_for: Option<DateTime<Utc>>,
    pub(crate) status_override: Option<TriggerStatus>,
}

/// A schedule's stored state as read before a republish, with its definition still undecoded.
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct StoredTriggerState {
    pub(crate) definition_json: serde_json::Value,
    pub(crate) started_at: DateTime<Utc>,
    pub(crate) last_tick_for: Option<DateTime<Utc>>,
    pub(crate) status_override: Option<TriggerStatus>,
}

/// Only the calendar times of a stored definition, read leniently to report which a republish drops when the rest no longer decodes.
#[derive(Clone, Debug, Default, Deserialize)]
pub(crate) struct StoredCalendarTimes {
    #[serde(default)]
    pub(crate) schedule: StoredSchedule,
}

/// A stored schedule's calendar times; any other kind has none.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum StoredSchedule {
    Calendar {
        #[serde(default)]
        times: Vec<DateTime<Utc>>,
    },
    #[default]
    #[serde(other)]
    Other,
}

/// Each member job's waits, why it waits (the first lineage edge found, as node and upstream), and its cycles.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GroupWaits {
    pub edges: BTreeMap<String, BTreeSet<String>>,
    pub reasons: BTreeMap<(String, String), (String, String)>,
    pub cycles: Vec<GroupCycle>,
}

/// Members that wait on each other, named as one cycle among them with a reason per wait.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupCycle {
    pub description: String,
    /// Whether a wait in the cycle reads a seed or a node that more than one member builds.
    pub shared_read: bool,
}

/// Providers the current snapshot declares; empty before a publish or when the project has none.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ProvidersRead {
    pub snapshot_id: Option<uuid::Uuid>,
    pub providers: Vec<ProviderRecord>,
}

/// A provider as its location published it, from the class alone: never a setting value.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ProviderRecord {
    pub name: String,
    /// The class as `module.qualname`.
    pub class: String,
    /// The class docstring.
    pub description: String,
    /// The file declaring the class, relative to the project.
    pub path: String,
    pub settings: Vec<ProviderSetting>,
    /// Code that injects the provider, in discovery order.
    pub used_by: Vec<ProviderConsumer>,
    /// Snapshot nodes that run that code: Python nodes, models calling a hook or using a materialization, and sources a loader fills.
    #[serde(default)]
    pub nodes: Vec<String>,
    /// Jobs whose selection includes one of those nodes.
    #[serde(default)]
    pub jobs: Vec<String>,
    /// Each of those jobs with the provider's nodes it selects, in node order.
    #[serde(default)]
    pub job_nodes: std::collections::BTreeMap<String, Vec<String>>,
}

/// One declared setting field; its value stays in the location's environment.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ProviderSetting {
    pub name: String,
    /// The annotation as written, such as `str` or `SecretStr | None`.
    #[serde(rename = "type")]
    pub type_name: String,
    pub required: bool,
    /// Whether the annotation is a pydantic secret type.
    pub secret: bool,
    /// The environment variable the setting is read from.
    pub env: String,
    pub description: Option<String>,
    /// Whether this required secret is unset in the location's environment.
    #[serde(default)]
    pub missing: bool,
}

/// Code that injects a provider: a node (`loader`, `asset`, `check`, `hook` or `materialization`, keyed `kind:name`), an `event_exporter` or a `sink`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ProviderConsumer {
    pub kind: String,
    pub name: String,
}
