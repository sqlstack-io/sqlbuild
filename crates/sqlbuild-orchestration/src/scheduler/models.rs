//! Values owned by the scheduler, independent of Python and runtime timers.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::declarations::models::{JobDefinition, TriggerDefinition, TriggerStatus};

/// A holder must present the exact epoch for every scheduler write.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchedulerLease {
    pub holder: Uuid,
    pub epoch: i64,
    pub expires_at: DateTime<Utc>,
}

/// Process identity survives cycles; the database remains authoritative for leadership.
#[derive(Clone, Debug)]
pub struct SchedulerState {
    pub(crate) holder: Uuid,
    pub(crate) lease: Option<SchedulerLease>,
}

impl SchedulerState {
    #[must_use]
    pub const fn new(holder: Uuid) -> Self {
        Self {
            holder,
            lease: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SchedulerCycle {
    pub leader: bool,
    pub evaluated: usize,
    pub ticks: usize,
    pub unresponsive: usize,
    pub wait_for: std::time::Duration,
}

/// Validated at the native boundary and evaluated as wall-clock times with SQLBuild's DST policy.
#[derive(Clone, Debug)]
pub struct CronSchedule {
    pub(crate) expression: croner::Cron,
    pub(crate) timezone: chrono_tz::Tz,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CatchUp {
    #[default]
    Latest,
    /// Process the oldest bounded batch; later due times remain due for the next transaction.
    All {
        #[schema(value_type = usize, minimum = 1)]
        max_runs: std::num::NonZeroUsize,
    },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Overlap {
    #[default]
    Coalesce,
    Queue,
    Allow,
}

impl Overlap {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Coalesce => "coalesce",
            Self::Queue => "queue",
            Self::Allow => "allow",
        }
    }
}

/// Due times come from the persisted calendar or cron expansion, never elapsed sleep time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TickState {
    pub trigger_id: Uuid,
    pub enabled: bool,
    pub last_tick_for: Option<DateTime<Utc>>,
    pub due_times: Vec<DateTime<Utc>>,
    /// First omitted time in a bounded expansion; never advance the cursor past an unrecorded tick.
    pub continuation: Option<DateTime<Utc>>,
    pub catch_up: CatchUp,
    pub overlap: Overlap,
    /// Only unclaimed waiting work can cover a later tick; executing work cannot cover future data.
    pub waiting_run: Option<Uuid>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum TickDecision {
    Requested { run_id: Uuid },
    Covered { covered_by_run_id: Uuid },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TickAction {
    pub tick_id: Uuid,
    pub scheduled_for: DateTime<Utc>,
    pub decision: TickDecision,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TickPlan {
    pub actions: Vec<TickAction>,
    pub last_tick_for: Option<DateTime<Utc>>,
    pub next_due_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct DueTrigger {
    pub(crate) trigger_id: Uuid,
    pub(crate) location_id: Uuid,
    pub(crate) target: String,
    pub(crate) job_key: String,
    pub(crate) snapshot_id: Uuid,
    pub(crate) image_id: Option<String>,
    pub(crate) definition_json: TriggerDefinition,
    pub(crate) job_json: JobDefinition,
    pub(crate) run_kind: crate::runs::models::RunKind,
    pub(crate) started_at: DateTime<Utc>,
    pub(crate) last_tick_for: Option<DateTime<Utc>>,
    pub(crate) status: TriggerStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TickOutcome {
    Requested,
    Skipped,
    Covered,
    Failed,
    Deferred,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct TickRecord {
    pub tick_id: Uuid,
    pub trigger_id: Uuid,
    pub scheduled_for: DateTime<Utc>,
    pub evaluated_at: DateTime<Utc>,
    pub epoch: i64,
    pub outcome: TickOutcome,
    pub run_id: Option<Uuid>,
    pub covered_by_run_id: Option<Uuid>,
    pub reason: Option<String>,
}
