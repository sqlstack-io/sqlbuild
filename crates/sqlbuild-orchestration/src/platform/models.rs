//! Platform health is a typed stream of observations, independent of user code.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::models::ControlClient;
use crate::declarations::models::TriggerDefinition;
use crate::sensors::models::{CustomSensorOutcome, SensorEvaluationRequest};
use crate::warehouse_metadata::models::{WarehouseHistoryKind, WarehouseStateLocation};

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

/// Only named location operations cross this boundary, never arbitrary SQL or Python.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum LocationWork {
    CheckConnection {},
    EvaluateSensor {
        #[schema(value_type = Object)]
        evaluation: Box<SensorEvaluationRequest>,
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
    Connection { healthy: bool },
    SensorEvaluation { outcome: CustomSensorOutcome },
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
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PlatformEvent {
    Heartbeat {
        replica_id: Uuid,
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
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct InstallationHealth {
    pub(crate) installation_id: Uuid,
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
