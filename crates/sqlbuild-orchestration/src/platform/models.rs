//! Platform health is a typed stream of observations, independent of user code.

use crate::api::models::ControlClient;
use crate::credentials::models::AuditRetention;
use crate::declarations::models::TriggerDefinition;
use crate::discovered_sets::models::DiscoveredSetStatus;
use crate::platform::_helpers::health::current_instance::{CurrentInstance, current_instance};
use crate::runs::models::{RunKind, RunLogRetention, RunNodeFailure, RunOutcome};
use crate::sensors::models::{SensorEvaluationRequest, SensorFunctionOutcome};
use crate::snapshots::models::{SnapshotRecord, SnapshotRetention};
use crate::warehouse_metadata::models::StateHistoryRetention;
use crate::warehouse_metadata::models::{
    StatisticsRetention, WarehouseHistoryKind, WarehouseStateLocation,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

/// Every retention horizon one server's maintenance role enforces.
#[derive(Clone, Debug, Default)]
pub struct RetentionPolicies {
    pub audit: AuditRetention,
    pub statistics: StatisticsRetention,
    pub snapshots: SnapshotRetention,
    pub run_logs: RunLogRetention,
    pub state_history: StateHistoryRetention,
    /// The output store and how many days a finished run's output stays in it; `None` keeps no output.
    pub run_output: Option<(crate::run_output::models::OutputStore, u16)>,
}

/// One pooled outbound connection; the location never receives a store handle.
#[derive(Clone, Debug)]
pub struct LocationWorkClient {
    pub(crate) connection: ControlClient,
    pub(crate) work_url: reqwest::Url,
    pub(crate) location_id: Uuid,
}

/// Locations report bounded facts, never connection strings; a failed warehouse read carries only its redacted error text.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum LocationHealth {
    Heartbeat {},
    CompileFailed {
        generation: i64,
        /// The configured rules' findings when they failed the load; absent for other failures and from older locations.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rules: Option<LocationRuleFailure>,
        /// Why the load failed, redacted and bounded; absent from older locations.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    Connection {
        generation: i64,
        healthy: bool,
        /// Why the location checked; absent from older locations, which checked only after publishing.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        trigger: Option<ConnectionCheckTrigger>,
        /// How long the check took; absent from older locations.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        elapsed_millis: Option<u64>,
    },
    /// The providers whose required secrets the location's environment leaves unset.
    ProviderSecrets {
        generation: i64,
        /// Unset secrets of providers the target doesn't allow to miss them; above zero is a health problem.
        missing: u64,
        /// Every provider missing a secret, including those the target allows; names only, never values.
        providers: Vec<MissingProviderSecrets>,
    },
    /// The discovered sets a publication used; a failed set keeps serving its last good items.
    Discovery {
        generation: i64,
        sets: Vec<DiscoveryHealth>,
    },
    /// The location's plan of what changed on the current snapshot failed, derived by the server from that plan, never reported by a location.
    ChangePlanFailed {
        generation: i64,
        reason: String,
    },
    /// Jobs and groups this server's `pool_caps` clamp, derived by the server, never reported by a location.
    PoolCapped {
        generation: i64,
        jobs: Vec<PoolCapWarning>,
    },
    WarehouseSync {
        generation: i64,
        location: WarehouseStateLocation,
        table: WarehouseHistoryKind,
        failure: Option<WarehouseSyncFailure>,
        records_sent: u64,
        elapsed_millis: u64,
        /// A failed read's warehouse error, redacted and bounded; absent otherwise.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
        /// Why a read failed, in closed terms; the location's log has the warehouse's message.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        read_failure: Option<WarehouseReadFailure>,
        /// Rows, or statistics relations, the failed read asked for.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        page_limit: Option<u64>,
        /// Where the synchronization this report belongs to has got to; absent from older locations.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        progress: Option<WarehouseSyncProgress>,
        /// On the report that completes or stops a sync, when the next safety sync is due unless a run ends first; absent otherwise, where no timed sync follows, and from older locations.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        next_sync_at: Option<DateTime<Utc>>,
        /// On the report of a table's last delivered page, the records the server then mirrored of that table across the target's state locations.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mirrored_records: Option<u64>,
    },
}

/// A location's published generation, as its interpreter hands it to the supervisor.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublishedGeneration {
    pub record: SnapshotRecord,
    /// The earliest calendar refresh deadline.
    pub refresh_at: Option<DateTime<Utc>>,
    /// The state locations its nodes write.
    pub state_locations: Vec<WarehouseStateLocation>,
    /// The providers whose required secrets the location's environment leaves unset.
    pub missing_secrets: Vec<MissingProviderSecrets>,
    /// For each provider missing secrets, the node keys that use it and then those that read them.
    pub secrets_needed_by: BTreeMap<String, Vec<String>>,
}

/// A provider whose required secrets are unset: what is missing and which nodes need it, never a value.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct MissingProviderSecrets {
    pub provider: String,
    /// The provider's file, relative to the project directory.
    pub path: String,
    pub settings: Vec<MissingProviderSetting>,
    /// Snapshot node keys that run the provider, such as `source:raw_payments`.
    pub nodes: Vec<String>,
    /// The target's `allow_missing_secrets` lists the provider, so its location shows a note, not a problem.
    pub allowed: bool,
}

/// One unset required setting: the field, the variable it reads and the field's line in the provider file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct MissingProviderSetting {
    pub field: String,
    pub variable: String,
    pub line: Option<u64>,
}

/// One discovered set as a location's health names it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct DiscoveryHealth {
    pub name: String,
    pub status: DiscoveredSetStatus,
    /// Why the newest attempt failed, secrets masked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// How many items the set serves: its newest result, or the last good one after a failure.
    pub items: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_good_at: Option<DateTime<Utc>>,
}

/// The stage a warehouse sync failed at; only a failed read also reports its redacted error.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum WarehouseSyncFailure {
    Watermark,
    Read,
    Delivery,
    /// Another process, such as a DuckDB build, holds the warehouse file; the sync waits for it.
    Busy,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct LocationHealthReport {
    pub event_id: Uuid,
    pub session_id: Uuid,
    pub target: String,
    pub health: LocationHealth,
}

/// Narrows a location's health history to one target, so other targets' traffic cannot crowd it out.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocationHealthQuery {
    pub target: Option<String>,
}

/// Receipt time belongs to the server; consumers derive freshness rather than storing availability.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct LocationHealthObservation {
    pub received_at: DateTime<Utc>,
    pub report: LocationHealthReport,
}

/// One target's state from its newest health observations, as the web UI shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum LocationState {
    /// The newest instance has not finished its first load; it reports nothing yet, and earlier instances don't count.
    Starting,
    Healthy,
    /// The warehouse connection check after the last load failed.
    Degraded,
    /// The last load failed, so runs use the previous snapshot.
    FallingBack,
    /// The plan of what changed on the current snapshot failed, so what needs building is unknown.
    ChangePlanFailed,
    /// A provider secret is unset in the location's environment, so runs that use the provider fail.
    MissingSecrets,
    /// A discover function failed at the last publication, so its set serves its last good items or none.
    DiscoveryFailed,
    /// No instance has checked in for 90 seconds.
    Broken,
}

impl LocationState {
    /// `target`'s state at `now`: down without a recent check-in, starting until the newest instance's first load finishes, then from that instance's reports a failed load (the newest load report by arrival decides, as failed loads keep the generation they tried), missing secrets, the server's record of a failed plan of what changed, a failed discover function, then a failed connection.
    #[must_use]
    pub fn of(
        observations: &[LocationHealthObservation],
        (target, now): (&str, DateTime<Utc>),
    ) -> Self {
        let own: Vec<&LocationHealthObservation> = observations
            .iter()
            .filter(|observation| observation.report.target == target)
            .collect();
        let silence =
            chrono::TimeDelta::seconds(crate::platform::constants::LOCATION_SILENCE_SECONDS);
        if !own.iter().any(|observation| {
            !matches!(observation.report.health, LocationHealth::PoolCapped { .. })
                && now - observation.received_at <= silence
        }) {
            return Self::Broken;
        }
        let change_plan_failed: bool = own.iter().any(|observation| {
            matches!(
                observation.report.health,
                LocationHealth::ChangePlanFailed { .. }
            )
        });
        let own: Vec<&LocationHealthObservation> =
            match current_instance((&own, observations.len()), (silence, now)) {
                CurrentInstance::Starting => return Self::Starting,
                CurrentInstance::Loaded(session) => own
                    .into_iter()
                    .filter(|observation| observation.report.session_id == session)
                    .collect(),
                CurrentInstance::Unknown => own,
            };
        let mut load: Option<(DateTime<Utc>, LoadReport)> = None;
        let mut secrets: Option<(DateTime<Utc>, u64)> = None;
        let mut discovery: Option<(DateTime<Utc>, bool)> = None;
        for observation in own {
            let seen = observation.received_at;
            match observation.report.health {
                LocationHealth::CompileFailed { .. } if load.is_none_or(|(at, _)| seen > at) => {
                    load = Some((seen, LoadReport::Failed));
                }
                LocationHealth::Connection { healthy, .. }
                    if load.is_none_or(|(at, _)| seen > at) =>
                {
                    load = Some((seen, LoadReport::Published { connected: healthy }));
                }
                LocationHealth::ProviderSecrets { missing, .. }
                    if secrets.is_none_or(|(at, _)| seen > at) =>
                {
                    secrets = Some((seen, missing));
                }
                LocationHealth::Discovery { ref sets, .. }
                    if discovery.is_none_or(|(at, _)| seen > at) =>
                {
                    let failing: bool = sets
                        .iter()
                        .any(|set| set.status == DiscoveredSetStatus::Failed);
                    discovery = Some((seen, failing));
                }
                _ => {}
            }
        }
        match (load.map(|(_, report)| report), secrets) {
            (Some(LoadReport::Failed), _) => Self::FallingBack,
            (_, Some((_, missing))) if missing > 0 => Self::MissingSecrets,
            _ if change_plan_failed => Self::ChangePlanFailed,
            _ if discovery.is_some_and(|(_, failing)| failing) => Self::DiscoveryFailed,
            (Some(LoadReport::Published { connected: false }), _) => Self::Degraded,
            _ => Self::Healthy,
        }
    }
}

/// How the newest load a location reported ended.
#[derive(Clone, Copy)]
enum LoadReport {
    Failed,
    Published { connected: bool },
}

/// Only named location operations cross this boundary, never arbitrary SQL or Python.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum LocationWork {
    CheckConnection {},
    /// Read SQLBuild's state and catalog back from the warehouse now, not only after a run ends.
    SyncWarehouse {},
    EvaluateSensor {
        #[schema(value_type = Object)]
        evaluation: Box<SensorEvaluationRequest>,
    },
    /// `sqb plan --json` on exact node keys the server froze from a person's build command.
    Plan {
        command: Vec<String>,
        selection: std::collections::BTreeSet<String>,
        #[schema(value_type = Object)]
        settings: std::collections::BTreeMap<String, serde_json::Value>,
        /// Plan again even when the location kept a plan for the same request.
        fresh: bool,
        /// The jobs the command names, pinned as an ad-hoc build of it pins them.
        #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
        jobs: std::collections::BTreeMap<String, crate::runs::models::PinnedJob>,
        /// The settings the command gives, `warehouse` included though a plan never takes it; they replace the pinned jobs' own.
        #[serde(default, skip_serializing_if = "std::collections::BTreeSet::is_empty")]
        given: std::collections::BTreeSet<String>,
    },
    /// Compile and publish again, calling every discover function, as `r` does in `sqb dev`.
    Republish {
        /// The name of whoever asked, which the location prints with the request's reason.
        requested_by: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct LocationWorkRequest {
    pub request_id: Uuid,
    pub location_id: Uuid,
    pub target: String,
    pub reason: String,
    pub work: LocationWork,
}

/// A person's build command to plan on the location; the server freezes it like an ad-hoc run.
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanRequest {
    pub request_id: Uuid,
    pub command: Vec<String>,
    /// Plan again instead of returning the plan the location kept for the same request.
    pub fresh: bool,
}

/// A person asks a location for one named read-only operation, such as a connection test.
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PersonWorkRequest {
    pub request_id: Uuid,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct LocationWorkPoll {
    pub session_id: Uuid,
    pub target: String,
    /// Which work this poll takes.
    pub scope: LocationWorkScope,
}

/// Which work a location takes in one poll.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum LocationWorkScope {
    /// Every kind of work, while the location serves a publication of its current files.
    All,
    /// Only republishes, while the location has no publication of its current files.
    RepublishOnly,
}

impl LocationWorkScope {
    /// The scope's name as the claim query compares it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::RepublishOnly => "republish_only",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct LocationWorkClaim {
    pub request: LocationWorkRequest,
    pub generation: i64,
    pub claim_id: Uuid,
    pub session_id: Uuid,
    pub lease_expires_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum LocationWorkOutcome {
    Connection {
        healthy: bool,
    },
    SensorEvaluation {
        outcome: SensorFunctionOutcome,
    },
    /// The plan document `sqb plan --json` printed, or its `{"error": ...}` report.
    Plan {
        #[schema(value_type = Object)]
        document: serde_json::Value,
        /// Model diff text, which the document never carries; stored apart, never read back here.
        #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
        diffs: std::collections::BTreeMap<String, PlanModelDiff>,
    },
    /// The location took the request; its next metadata sync reads the warehouse.
    SyncStarted {},
    /// The republish published this snapshot.
    Published {
        snapshot_id: Uuid,
        generation: i64,
    },
    /// The republish compiled nothing to publish; the last good snapshot keeps serving.
    PublishFailed {
        generation: i64,
        error: String,
    },
    Superseded {},
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct LocationWorkResult {
    pub claim_id: Uuid,
    pub session_id: Uuid,
    pub outcome: LocationWorkOutcome,
}

/// Expiry is derived from the deadline; only requests, leases and completed results are stored.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct LocationWorkRecord {
    pub request_json: LocationWorkRequest,
    pub requested_by: String,
    pub requested_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub generation: i64,
    pub result_json: Option<LocationWorkResult>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct StoredLocationWork {
    #[serde(flatten)]
    pub(crate) record: LocationWorkRecord,
    pub(crate) claim_id: Option<Uuid>,
    pub(crate) session_id: Option<Uuid>,
    pub(crate) claimed_until: Option<DateTime<Utc>>,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, utoipa::ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum PlatformEventKind {
    Heartbeat,
    SchedulerNoLeader,
    TickOverdue,
    CalendarHorizonLow,
    LocationBroken,
    LocationFailing,
    LaunchFailed,
    RunFailed,
    TriggerSkipped,
    SensorRechecked,
    CalendarTimesDropped,
}

impl PlatformEventKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Heartbeat => "heartbeat",
            Self::SchedulerNoLeader => "scheduler_no_leader",
            Self::TickOverdue => "tick_overdue",
            Self::CalendarHorizonLow => "calendar_horizon_low",
            Self::LocationBroken => "location_broken",
            Self::LocationFailing => "location_failing",
            Self::LaunchFailed => "launch_failed",
            Self::RunFailed => "run_failed",
            Self::TriggerSkipped => "trigger_skipped",
            Self::SensorRechecked => "sensor_rechecked",
            Self::CalendarTimesDropped => "calendar_times_dropped",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PlatformEvent {
    Heartbeat {
        replica_id: Uuid,
        /// The control store's database size on disk when the replica checked in.
        store_bytes: i64,
    },
    SchedulerNoLeader,
    TickOverdue {
        trigger_id: Uuid,
        scheduled_for: DateTime<Utc>,
    },
    CalendarHorizonLow {
        trigger_id: Uuid,
        refresh_at: DateTime<Utc>,
        horizon_end: Option<DateTime<Utc>>,
    },
    /// All observed instances of this location/target have missed three heartbeats.
    LocationBroken {
        location_id: Uuid,
        target: String,
        last_seen_at: DateTime<Utc>,
    },
    /// A checking-in location's latest load, plan of what changed, discovery or connection check failed; `serving_generation` is what runs use instead.
    LocationFailing {
        location_id: Uuid,
        target: String,
        state: LocationState,
        failed_at: DateTime<Utc>,
        error: Option<String>,
        serving_generation: Option<i64>,
    },
    /// A launch attempt or its response failed; the worker may still start or finish.
    LaunchFailed {
        run_id: Uuid,
        claim_id: Uuid,
        code: String,
    },
    /// A job, group or ad-hoc run ended failed or partial; only the webhook receives it, never person reads.
    RunFailed {
        run_id: Uuid,
        run_kind: RunKind,
        job_key: Option<String>,
        location_id: Uuid,
        target: String,
        outcome: RunOutcome,
        /// The first failed nodes by key, with redacted and bounded error text.
        failed_nodes: Vec<RunNodeFailure>,
        failed_node_count: i64,
    },
    /// A trigger was skipped because its stored job or pinned snapshot can't be read, or a sensor because its check failed; `reason` says which.
    TriggerSkipped {
        trigger_id: Uuid,
        snapshot_id: Uuid,
        reason: String,
    },
    /// A sensor's last check was recorded by an earlier release in a form that can't be read, so evaluation `evaluation_id` checked it again from scratch; withdrawn once a check completes.
    SensorRechecked {
        trigger_id: Uuid,
        snapshot_id: Uuid,
        evaluation_id: Uuid,
    },
    /// A republish dropped a calendar's pending past-due `times`, because its previous definition, published by an earlier release, can't be read; withdrawn at the trigger's next successful tick.
    CalendarTimesDropped {
        trigger_id: Uuid,
        snapshot_id: Uuid,
        times: Vec<DateTime<Utc>>,
    },
}

impl PlatformEvent {
    #[must_use]
    pub const fn kind(&self) -> PlatformEventKind {
        match self {
            Self::Heartbeat { .. } => PlatformEventKind::Heartbeat,
            Self::SchedulerNoLeader => PlatformEventKind::SchedulerNoLeader,
            Self::TickOverdue { .. } => PlatformEventKind::TickOverdue,
            Self::CalendarHorizonLow { .. } => PlatformEventKind::CalendarHorizonLow,
            Self::LocationBroken { .. } => PlatformEventKind::LocationBroken,
            Self::LocationFailing { .. } => PlatformEventKind::LocationFailing,
            Self::LaunchFailed { .. } => PlatformEventKind::LaunchFailed,
            Self::RunFailed { .. } => PlatformEventKind::RunFailed,
            Self::TriggerSkipped { .. } => PlatformEventKind::TriggerSkipped,
            Self::SensorRechecked { .. } => PlatformEventKind::SensorRechecked,
            Self::CalendarTimesDropped { .. } => PlatformEventKind::CalendarTimesDropped,
        }
    }

    #[must_use]
    pub const fn event_type(&self) -> &'static str {
        match self {
            Self::Heartbeat { .. } => "platform_heartbeat",
            Self::SchedulerNoLeader => "platform_scheduler_no_leader",
            Self::TickOverdue { .. } => "platform_tick_overdue",
            Self::CalendarHorizonLow { .. } => "platform_calendar_horizon_low",
            Self::LocationBroken { .. } => "platform_location_broken",
            Self::LocationFailing { .. } => "platform_location_failing",
            Self::LaunchFailed { .. } => "platform_launch_failed",
            Self::RunFailed { .. } => "platform_run_failed",
            Self::TriggerSkipped { .. } => "platform_trigger_skipped",
            Self::SensorRechecked { .. } => "platform_sensor_rechecked",
            Self::CalendarTimesDropped { .. } => "platform_calendar_times_dropped",
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct InstallationHealth {
    pub(crate) installation_id: Uuid,
    /// Distinguishes one leaderless outage from the next.
    pub(crate) lease_expires_at: Option<DateTime<Utc>>,
    pub(crate) leader: i64,
}

#[derive(Debug, Deserialize)]
pub(crate) struct WatchdogTrigger {
    pub(crate) trigger_id: Uuid,
    pub(crate) definition_json: TriggerDefinition,
    pub(crate) next_due_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct WatchdogLocation {
    pub(crate) location_id: Uuid,
    pub(crate) target: String,
    pub(crate) last_seen_at: DateTime<Utc>,
}

/// A stored platform event a person may see; the location target is absent for server-wide events.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct PersonPlatformEvent {
    pub storage_id: i64,
    pub event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
    pub event: PlatformEvent,
    pub location_id: Option<Uuid>,
    pub target: Option<String>,
}

/// One scan of stored platform events, keeping those the person may see.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct PersonPlatformEventPage {
    pub events: Vec<PersonPlatformEvent>,
    /// The `before` that continues the scan, absent once no older events remain; a page can be empty and still continue.
    pub before: Option<i64>,
}

/// The location target a trigger or run belongs to.
#[derive(Debug, Deserialize)]
pub(crate) struct EventScope {
    pub(crate) id: Uuid,
    pub(crate) location_id: Uuid,
    pub(crate) target: String,
}

/// Stable event identity permits receiver-side deduplication after uncertain acknowledgements.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlatformEventEnvelope {
    pub storage_id: i64,
    pub event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
    pub payload: PlatformEventPayload,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlatformEventPayload {
    pub platform: PlatformEvent,
}

/// Problems derived from live state when read; nothing here is stored.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct PlatformProblems {
    pub scheduler_leader: bool,
    /// When the last scheduler lease ran out, or will run out; absent until a server first leads.
    pub scheduler_lease_expires_at: Option<DateTime<Utc>>,
    pub broken_locations: Vec<BrokenLocation>,
    /// Checking-in locations whose latest load, plan of what changed, discovery or connection check failed.
    #[serde(default)]
    pub failing_locations: Vec<FailingLocation>,
    pub overdue_ticks: Vec<OverdueTick>,
    /// Running calendar schedules whose dates run out within a day.
    #[serde(default)]
    pub low_calendars: Vec<LowCalendar>,
    /// Triggers skipped because their stored job or pinned snapshot can't be read, until their location publishes again.
    #[serde(default)]
    pub skipped_triggers: Vec<SkippedTrigger>,
    /// Calendars whose pending times a republish dropped, until their next successful tick.
    #[serde(default)]
    pub dropped_calendar_times: Vec<DroppedCalendarTimes>,
    /// Sensors checked again from scratch because their last check can't be read, until a check completes.
    #[serde(default)]
    pub rechecked_sensors: Vec<RecheckedSensor>,
}

/// A calendar whose pending past-due times a republish dropped, because its previous definition can't be read; it clears at its next successful tick.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct DroppedCalendarTimes {
    pub trigger_id: Uuid,
    pub name: String,
    pub location_id: Uuid,
    pub location_name: String,
    pub target: String,
    pub dropped_at: DateTime<Utc>,
    /// The times that were due by the republish but never ticked, oldest first.
    pub times: Vec<DateTime<Utc>>,
}

/// The live trigger problems, each list with its own withdrawal.
#[derive(Debug, Default)]
pub(crate) struct TriggerProblems {
    pub(crate) skipped: Vec<SkippedTrigger>,
    pub(crate) rechecked: Vec<RecheckedSensor>,
    pub(crate) dropped: Vec<DroppedCalendarTimes>,
}

/// A sensor whose last check, recorded by an earlier release, can't be read, so it is checked again from scratch; it clears once a check completes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct RecheckedSensor {
    pub trigger_id: Uuid,
    pub name: String,
    pub location_id: Uuid,
    pub location_name: String,
    pub target: String,
    pub rechecked_at: DateTime<Utc>,
}

/// A trigger the scheduler skips because its stored job or pinned snapshot can't be read, or a sensor whose check failed; it clears once its location publishes again or, for a sensor, a later check completes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct SkippedTrigger {
    pub trigger_id: Uuid,
    pub name: String,
    pub location_id: Uuid,
    pub location_name: String,
    pub target: String,
    /// When it was first skipped for this snapshot.
    pub skipped_at: DateTime<Utc>,
    pub reason: String,
}

/// A live trigger and the snapshot it was published in.
#[derive(Debug, Deserialize)]
pub(crate) struct PublishedTrigger {
    pub(crate) trigger_id: Uuid,
    pub(crate) name: String,
    pub(crate) location_id: Uuid,
    pub(crate) location_name: String,
    pub(crate) target: String,
    pub(crate) snapshot_id: Uuid,
}

/// A running calendar schedule whose last date is at most a day away, or which has none left.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct LowCalendar {
    pub trigger_id: Uuid,
    pub name: String,
    pub location_id: Uuid,
    pub location_name: String,
    pub target: String,
    /// When the location next works out the calendar's dates.
    pub refresh_at: DateTime<Utc>,
    /// The calendar's last date; absent when none are left.
    pub horizon_end: Option<DateTime<Utc>>,
}

/// A running schedule of a live location, with its declaration.
#[derive(Debug, Deserialize)]
pub(crate) struct RunningSchedule {
    pub(crate) trigger_id: Uuid,
    pub(crate) name: String,
    pub(crate) location_id: Uuid,
    pub(crate) location_name: String,
    pub(crate) target: String,
    pub(crate) definition_json: TriggerDefinition,
}

/// Every instance of this location target has missed two check-ins.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct BrokenLocation {
    pub location_id: Uuid,
    pub location_name: String,
    pub target: String,
    pub last_seen_at: DateTime<Utc>,
}

/// A checking-in location whose latest load, plan of what changed, discovery or connection check failed, and what it serves instead.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct FailingLocation {
    pub location_id: Uuid,
    pub location_name: String,
    pub target: String,
    /// `falling_back`, `change_plan_failed`, `discovery_failed` or `degraded`.
    pub state: LocationState,
    /// When the failure was reported.
    pub failed_at: DateTime<Utc>,
    /// What failed, redacted; absent when an older location reported no text.
    pub error: Option<String>,
    /// The snapshot the target serves; absent before its first publication.
    pub serving: Option<SnapshotRecord>,
}

/// A running schedule whose next tick is more than 30 seconds late.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct OverdueTick {
    pub trigger_id: Uuid,
    pub name: String,
    pub location_id: Uuid,
    pub location_name: String,
    pub target: String,
    pub scheduled_for: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SchedulerLease {
    pub(crate) expires_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct LateTarget {
    pub(crate) location_id: Uuid,
    pub(crate) target: String,
}

/// Problems found at one instant: the scheduler lease, broken location targets and targets with late ticks.
#[derive(Debug)]
pub(crate) struct ProblemScan {
    pub(crate) now: DateTime<Utc>,
    pub(crate) lease: Option<SchedulerLease>,
    pub(crate) broken: Vec<BrokenLocation>,
    pub(crate) failing: Vec<FailingLocation>,
    pub(crate) late_targets: std::collections::BTreeSet<(Uuid, String)>,
    pub(crate) low_calendars: Vec<LowCalendar>,
    pub(crate) skipped_triggers: Vec<SkippedTrigger>,
    pub(crate) rechecked_sensors: Vec<RecheckedSensor>,
    pub(crate) dropped_calendar_times: Vec<DroppedCalendarTimes>,
}

/// Where the control store keeps its data.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum StorageKind {
    Sqlite,
    Postgresql,
}

/// The control store's installation and schema, as recorded when it was created.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct InstallationRecord {
    pub storage: StorageKind,
    pub installed_at: DateTime<Utc>,
    /// Digest of the schema; a server starts only on the exact schema it creates.
    pub schema_checksum: String,
    pub schema_applied_at: DateTime<Utc>,
    /// The database's size on disk now: SQLite's main file, or the whole PostgreSQL database.
    pub store_bytes: i64,
}

/// The stored installation and schema rows.
#[derive(Debug, Deserialize)]
pub(crate) struct StoredInstallation {
    pub(crate) installed_at: DateTime<Utc>,
    pub(crate) schema_checksum: String,
    pub(crate) schema_applied_at: DateTime<Utc>,
}

/// A job or group asking for more of a pool than this server's cap, so it runs alone in that pool.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PoolCapWarning {
    pub job_key: String,
    pub kind: crate::runs::models::RunKind,
    pub pool: String,
    pub amount: u32,
    pub cap: u32,
    /// Such as "job X takes 3 of pool warehouse_heavy; this server caps it at 1, so it runs alone in that pool".
    pub message: String,
}

/// When a location checks its warehouse connection on its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionCheckTrigger {
    /// The first check after the location process starts and publishes.
    Startup,
    /// After the location published a later snapshot.
    Publish,
}

/// How far one synchronization has got, so people see it running and when it completed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct WarehouseSyncProgress {
    /// The reported kind's position in the synchronization, from 1.
    pub position: u64,
    /// Kinds the synchronization reads: 14 for each state schema.
    pub kinds: u64,
    /// Rows the synchronization has delivered, this report's included.
    pub rows: u64,
    /// Milliseconds since the synchronization began.
    pub elapsed_millis: u64,
    /// This report's page was the synchronization's last, so it has completed.
    pub completed: bool,
    /// Failed reads in a row stopped the sync until a run, a request, a publication or `next_sync_at`.
    #[serde(default)]
    pub halted: bool,
}

/// What went wrong reading one page, without the warehouse's message, which may name hosts or users.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WarehouseReadFailure {
    /// The reader did not start or connect to the warehouse, or the read raised an error.
    ReadError {},
    /// The reader did not answer within this many seconds.
    TimedOut { seconds: u64 },
}

/// Configured rules that failed a location's load: how many findings, and the first of them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct LocationRuleFailure {
    pub finding_count: u64,
    /// At most `MAX_REPORTED_RULE_FINDINGS`, in the rules' order.
    pub findings: Vec<LocationRuleFinding>,
}

/// One rule finding as `sqb compile` prints it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct LocationRuleFinding {
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u64>,
}

/// A plan section, read a page at a time beside its summary, in the order the plan reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum PlanRowSection {
    /// Built models the plan rebuilds in full.
    FullRefresh,
    QueryChanged,
    FunctionChanged,
    ConfigChanged,
    SchemaChanged,
    /// Built models with no recorded build.
    FirstRun,
    Renamed,
    /// Built models with no change of their own.
    RoutineModels,
    Migrations,
    ColumnMigrations,
    TableTypeConversions,
    Retention,
    LoaderReloads,
    LoaderLoads,
    SourceReloads,
    SourceLoads,
    /// Functions whose definition changed.
    ChangedFunctions,
    /// Functions without a change.
    Functions,
    /// Seeds the run loads for the first time.
    FirstRunSeeds,
    /// Seeds whose definition or settings changed.
    ChangedSeeds,
    /// Seeds the run loads or reports on without a change of their own.
    UnchangedSeeds,
    PythonNodes,
    /// `--changed` nodes held back for a job's own run.
    HeldBack,
    /// `--changed` nodes that run only when something they read changed.
    Unchanged,
    Warnings,
}

/// One page of a section's rows whose name contains `name`, ignoring case; `offset` defaults to 0 and `limit` to the cap.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanRowsQuery {
    pub section: PlanRowSection,
    pub offset: Option<usize>,
    pub limit: Option<usize>,
    pub name: Option<String>,
}

/// A page of a plan section with the `--changed` runs, warnings and cursor window starts of its models.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanRows {
    /// Rows matching the name filter; the section's size without one.
    pub total: usize,
    /// Rows in the section, filter or not.
    pub section_total: usize,
    /// Model rows carry `diff_stats` in place of their diff text, which `plans/{request_id}/diff` serves.
    #[schema(value_type = Vec<Object>)]
    pub rows: Vec<serde_json::Value>,
    #[schema(value_type = Vec<Object>)]
    pub runs: Vec<serde_json::Value>,
    #[schema(value_type = Vec<Object>)]
    pub warnings: Vec<serde_json::Value>,
    pub cursor_window_starts: std::collections::BTreeMap<String, String>,
}

/// One planned model's diff text, as `sqb plan` prints it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanModelDiff {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query_diff: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_diff: Option<Vec<String>>,
}

/// The model whose diff to read.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanDiffQuery {
    pub model: String,
}

/// A planned model's diff text, stored beside its plan document.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanDiffRead {
    pub model: String,
    #[serde(flatten)]
    pub diff: PlanModelDiff,
}

/// A plan request's state with its summary: changed models in full, large sections as counts to page.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanSummaryRead {
    pub expires_at: DateTime<Utc>,
    /// `None` until the location answers; a plan's document is its summary.
    pub outcome: Option<LocationWorkOutcome>,
    /// How long the location took to plan, step by step; `None` until it planned, or when planning failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timings: Option<PlanTimings>,
}

/// How long one plan took: its steps in pipeline order with `Other` last, and what it read.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanTimings {
    pub total_seconds: f64,
    pub steps: Vec<PlanTimingStep>,
    /// `synced_state` for a preview over the synced warehouse state; `live` when it read the warehouse.
    pub planned_from: String,
    /// When the plan the location kept, and answered with again, was made; `None` for a new plan.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reused_from: Option<DateTime<Utc>>,
}

/// One planning step and its wall time.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanTimingStep {
    pub name: String,
    pub seconds: f64,
}

/// Where a republish request stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RepublishState {
    /// Waiting for the location to take it, or to finish publishing.
    Pending,
    Published,
    Failed,
    /// No location took it before it expired.
    Expired,
}

/// A republish request and its result once the location has one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct RepublishStatus {
    pub request_id: Uuid,
    pub location_id: Uuid,
    pub target: String,
    pub state: RepublishState,
    pub requested_by: String,
    pub reason: String,
    pub requested_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// One discovered set of the current snapshot, as a deploy reads it back.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct DiscoveredSetReadback {
    pub name: String,
    pub status: DiscoveredSetStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fetched_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_good_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
    /// The tags of the result the set serves (its newest, or its last good one), each as text.
    pub tags: std::collections::BTreeMap<String, String>,
    pub items: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check_every_seconds: Option<u64>,
    /// The `for_each` assets that read the set.
    pub assets: Vec<String>,
    /// The node each served item became under each of `assets`, or each asset's placeholder.
    pub nodes: Vec<DiscoveredNodeReadback>,
}

/// The node one item of a discovered set became under one `for_each` asset.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct DiscoveredNodeReadback {
    /// The node key, `asset:<asset>[<item>]`.
    pub key: String,
    pub asset: String,
    /// The item's ID, or `*` for the placeholder of a set that never succeeded.
    pub step: String,
    /// The item's arguments; one that is not a string reads as its JSON text.
    pub args: std::collections::BTreeMap<String, String>,
}

/// The discovered sets of a target's current snapshot; empty when it saved none.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct DiscoveredReadback {
    pub snapshot_id: Uuid,
    pub generation: i64,
    pub published_at: DateTime<Utc>,
    pub sets: Vec<DiscoveredSetReadback>,
}

/// What a republish client sends: a new request, or a read of one that holds open while it is pending.
#[derive(Clone, Copy, Debug)]
pub enum RepublishCall<'a> {
    Request(&'a PersonWorkRequest),
    Read { request_id: Uuid, wait: u64 },
}
