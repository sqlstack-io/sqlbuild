//! Platform health is a typed stream of observations, independent of user code.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::models::ControlClient;
use crate::credentials::models::AuditRetention;
use crate::declarations::models::TriggerDefinition;
use crate::runs::models::{RunKind, RunLogRetention, RunNodeFailure, RunOutcome};
use crate::sensors::models::{CustomSensorOutcome, SensorEvaluationRequest};
use crate::snapshots::models::SnapshotRetention;
use crate::warehouse_metadata::models::{
    StatisticsRetention, WarehouseHistoryKind, WarehouseStateLocation,
};

/// Every retention horizon one server's maintenance role enforces.
#[derive(Clone, Copy, Debug, Default)]
pub struct RetentionPolicies {
    pub audit: AuditRetention,
    pub statistics: StatisticsRetention,
    pub snapshots: SnapshotRetention,
    pub run_logs: RunLogRetention,
}

/// One pooled outbound connection; the location never receives a store handle.
#[derive(Clone, Debug)]
pub struct LocationWorkClient {
    pub(crate) connection: ControlClient,
    pub(crate) work_url: reqwest::Url,
    pub(crate) location_id: Uuid,
}

/// Locations report bounded facts, never connection strings or user exception text.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum LocationHealth {
    Heartbeat {},
    CompileFailed {
        generation: i64,
    },
    Connection {
        generation: i64,
        healthy: bool,
    },
    WarehouseSync {
        generation: i64,
        location: WarehouseStateLocation,
        table: WarehouseHistoryKind,
        failure: Option<WarehouseSyncFailure>,
        records_sent: u64,
        elapsed_millis: u64,
    },
}

/// Closed failure stages keep warehouse errors and credentials out of health observations.
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

/// Receipt time belongs to the server; consumers derive freshness rather than storing availability.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct LocationHealthObservation {
    pub received_at: DateTime<Utc>,
    pub report: LocationHealthReport,
}

/// One target's state from its newest health observations, as the web UI shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocationState {
    Healthy,
    /// The warehouse connection check after the last load failed.
    Degraded,
    /// The last load failed, so runs use the previous snapshot.
    FallingBack,
    /// No instance has checked in for 90 seconds.
    Broken,
}

impl LocationState {
    /// `target`'s state at `now` from observations that may cover several targets: down without a recent check-in, then a failed load, then a failed connection.
    #[must_use]
    pub fn of(observations: &[LocationHealthObservation], (target, now): (&str, DateTime<Utc>)) -> Self {
        let own: Vec<&LocationHealthObservation> = observations
            .iter()
            .filter(|observation| observation.report.target == target)
            .collect();
        let silence = chrono::TimeDelta::seconds(crate::platform::constants::LOCATION_SILENCE_SECONDS);
        if !own.iter().any(|observation| now - observation.received_at <= silence) {
            return Self::Broken;
        }
        let mut failed: Option<(DateTime<Utc>, i64)> = None;
        let mut connection: Option<(DateTime<Utc>, i64, bool)> = None;
        for observation in own {
            let seen = observation.received_at;
            match observation.report.health {
                LocationHealth::CompileFailed { generation } if failed.is_none_or(|(at, _)| seen > at) => {
                    failed = Some((seen, generation));
                }
                LocationHealth::Connection { generation, healthy } if connection.is_none_or(|(at, _, _)| seen > at) => {
                    connection = Some((seen, generation, healthy));
                }
                _ => {}
            }
        }
        match (failed, connection) {
            (Some((_, failed)), Some((_, connected, _))) if failed >= connected => Self::FallingBack,
            (Some(_), None) => Self::FallingBack,
            (_, Some((_, _, false))) => Self::Degraded,
            _ => Self::Healthy,
        }
    }
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
        outcome: CustomSensorOutcome,
    },
    /// The plan document `sqb plan --json` printed, or its `{"error": ...}` report.
    Plan {
        #[schema(value_type = Object)]
        document: serde_json::Value,
    },
    /// The location took the request; its next metadata sync reads the warehouse.
    SyncStarted {},
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
    LaunchFailed,
    RunFailed,
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
            Self::LaunchFailed { .. } => PlatformEventKind::LaunchFailed,
            Self::RunFailed { .. } => PlatformEventKind::RunFailed,
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
            Self::LaunchFailed { .. } => "platform_launch_failed",
            Self::RunFailed { .. } => "platform_run_failed",
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
    pub overdue_ticks: Vec<OverdueTick>,
    /// Running calendar schedules whose dates run out within a day.
    #[serde(default)]
    pub low_calendars: Vec<LowCalendar>,
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
    pub(crate) late_targets: std::collections::BTreeSet<(Uuid, String)>,
    pub(crate) low_calendars: Vec<LowCalendar>,
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
