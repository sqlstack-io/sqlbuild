//! What a server knows of its own health, kept in memory so it answers while the control database can't.

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::store::models::ControlStore;

/// Which kind of control database failure an error is; the sentence people read follows from it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ControlCauseCase {
    /// The database host's name doesn't resolve.
    NameLookup,
    /// The connection was refused, timed out or dropped.
    Unreachable,
    /// The database refused the configured login.
    LoginRejected,
    /// The database has no free connections for this server.
    NoFreeConnections,
    /// The database is shutting down or starting up.
    Restarting,
    /// Every pooled connection stayed busy for the whole wait.
    PoolExhausted,
    /// Another process holds the SQLite database's lock.
    Locked,
    /// Any other control database error, read by its own message.
    Other,
}

/// A failed control database operation as it is reported: the sentence, its case and each cause's own message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControlCause {
    pub case: ControlCauseCase,
    /// What people read, such as "Can't reach the control database: db.internal doesn't resolve (DNS)".
    pub message: String,
    /// The error and each of its sources, by their own messages; for administrators and the server log.
    pub chain: Vec<String>,
    /// Whether the same operation may succeed when retried.
    pub transient: bool,
    /// The pool's wait ran out, which is a full pool or a database that can't be reached.
    pub(crate) waited_for_pool: bool,
}

impl ControlCause {
    /// This cause at `target`, whose pool is `pool_full` or not, with the sentence that names it.
    #[must_use]
    pub fn at(self, target: &ControlTarget, pool_full: bool) -> Self {
        let case = match self.case {
            ControlCauseCase::Unreachable | ControlCauseCase::PoolExhausted
                if self.waited_for_pool =>
            {
                if pool_full {
                    ControlCauseCase::PoolExhausted
                } else {
                    ControlCauseCase::Unreachable
                }
            }
            case => case,
        };
        Self {
            case,
            message: crate::health::_helpers::sentence::sentence(case, target, &self.chain[0]),
            ..self
        }
    }

    /// Whether the control database can't be used while this lasts: a failure a retry may outlast, or a refused login.
    #[must_use]
    pub fn unavailable(&self) -> bool {
        self.transient || self.case == ControlCauseCase::LoginRejected
    }
}

/// Where the control database is, for the sentences that name it; read from its URL, never its password.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ControlTarget {
    pub host: Option<String>,
    pub port: Option<u16>,
    pub user: Option<String>,
    /// The pool's connection limit.
    pub connections: u32,
    /// How long a request waits for a pooled connection, in seconds.
    pub wait_seconds: u64,
}

/// A part of the server whose health is reported.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, utoipa::ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum HealthComponent {
    Server,
    ControlDatabase,
    Scheduler,
    Location,
}

/// How a component, or the server as a whole, stands.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, utoipa::ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Healthy,
    /// Working, with a part that isn't: pages work and say what is affected.
    Degraded,
    /// The control database can't be used, so nothing that reads it works.
    Unavailable,
}

/// The newest failure of a component and the failures since it last worked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ComponentFailure {
    pub(crate) at: DateTime<Utc>,
    /// The first failure since the component last worked.
    pub(crate) since: DateTime<Utc>,
    pub(crate) reason: String,
    pub(crate) case: Option<ControlCauseCase>,
    pub(crate) chain: Vec<String>,
    /// The newest requests that failed this way, for finding them in the server log.
    pub(crate) request_ids: VecDeque<Uuid>,
}

/// One component's last success and last failure; it is failing while the failure is newer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ComponentState {
    pub(crate) last_success: Option<DateTime<Utc>>,
    pub(crate) last_failure: Option<ComponentFailure>,
}

/// A location as this server last saw it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SeenLocation {
    pub(crate) name: String,
    /// When any of its instances last reported; `None` before its first report.
    pub(crate) last_report: Option<DateTime<Utc>>,
}

/// Everything the health report is made from.
#[derive(Debug, Default)]
pub(crate) struct HealthState {
    pub(crate) target: ControlTarget,
    pub(crate) store: Option<ControlStore>,
    pub(crate) control_database: ComponentState,
    /// `None` when this server runs no scheduler.
    pub(crate) scheduler: Option<ComponentState>,
    pub(crate) locations: BTreeMap<Uuid, SeenLocation>,
}

/// One server's in-memory health, shared by its API, scheduler and control database probe.
#[derive(Clone, Debug)]
pub struct ServerHealth {
    pub(crate) state: Arc<Mutex<HealthState>>,
    pub(crate) version: String,
    pub(crate) started_at: DateTime<Utc>,
}

/// The server row of the health report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ServerSummary {
    pub version: String,
    pub started_at: DateTime<Utc>,
    pub uptime_seconds: i64,
}

/// One component as anyone may see it: its state, since when, and a short reason.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ComponentHealth {
    pub component: HealthComponent,
    /// What people call it: "Control database", "Scheduler", or a location's name.
    pub name: String,
    /// The location this row is about.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location_id: Option<Uuid>,
    pub status: HealthStatus,
    /// When it stopped being healthy.
    pub since: Option<DateTime<Utc>>,
    pub reason: Option<String>,
    /// The control database failure's case, when that is the reason.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub case: Option<ControlCauseCase>,
    /// When it last worked; for a location, its last report.
    pub last_success: Option<DateTime<Utc>>,
    pub last_failure: Option<DateTime<Utc>>,
}

/// `GET /api/health`: every component's state, readable without signing in and without the control database.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct HealthReport {
    /// The worst component's: unavailable when the control database is, degraded when anything else isn't healthy.
    pub status: HealthStatus,
    pub checked_at: DateTime<Utc>,
    /// How often a client should ask again while the server isn't healthy, in seconds.
    pub retry_seconds: u64,
    pub server: ServerSummary,
    pub components: Vec<ComponentHealth>,
}

/// One component's details for administrators: where it is, each cause's message, and the failed requests.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ComponentDetails {
    pub component: HealthComponent,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location_id: Option<Uuid>,
    /// The control database's host and port.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    pub error_chain: Vec<String>,
    pub request_ids: Vec<Uuid>,
}

/// `GET /api/health/details`: what administrators see beside the public report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct HealthDetails {
    pub components: Vec<ComponentDetails>,
}

/// An active location and its instances' last report, as the control database probe reads them.
#[derive(Debug, Deserialize)]
pub(crate) struct StoredLocationReport {
    pub(crate) location_id: Uuid,
    pub(crate) name: String,
    pub(crate) last_seen_at: Option<DateTime<Utc>>,
}

impl ComponentState {
    /// Whether its newest failure is newer than its newest success.
    pub(crate) fn failing(&self) -> bool {
        self.last_failure
            .as_ref()
            .is_some_and(|failure| self.last_success.is_none_or(|success| failure.at > success))
    }

    pub(crate) fn succeed(&mut self, at: DateTime<Utc>) {
        self.last_success = Some(self.last_success.map_or(at, |success| success.max(at)));
    }

    /// Record a failure and return since when the component has been failing.
    pub(crate) fn fail(&mut self, failure: ComponentFailure) -> DateTime<Utc> {
        let failing = self.failing();
        let mut failure = failure;
        if let Some(previous) = self.last_failure.take().filter(|_| failing) {
            failure.since = previous.since;
            let mut request_ids = previous.request_ids;
            request_ids.extend(failure.request_ids);
            while request_ids.len() > crate::health::constants::KEPT_REQUEST_IDS {
                request_ids.pop_front();
            }
            failure.request_ids = request_ids;
        }
        let since = failure.since;
        self.last_failure = Some(failure);
        since
    }
}

impl Default for ServerHealth {
    fn default() -> Self {
        Self::new("", false)
    }
}

impl ServerHealth {
    /// A server's health from its start, with a scheduler row when it runs one.
    #[must_use]
    pub fn new(version: &str, scheduler: bool) -> Self {
        Self {
            state: Arc::new(Mutex::new(HealthState {
                scheduler: scheduler.then(ComponentState::default),
                ..HealthState::default()
            })),
            version: version.to_owned(),
            started_at: Utc::now(),
        }
    }

    pub(crate) fn state(&self) -> std::sync::MutexGuard<'_, HealthState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Name the control database the sentences mention.
    pub fn set_target(&self, target: ControlTarget) {
        self.state().target = target;
    }

    /// The opened store, which names the database and whose pool tells a full pool from an unreachable database.
    pub fn attach_store(&self, store: &ControlStore) {
        let mut state = self.state();
        state.target = crate::health::_helpers::store_target::store_target(store);
        state.store = Some(store.clone());
    }

    /// `cause` at this server's control database.
    #[must_use]
    pub fn locate(&self, cause: ControlCause) -> ControlCause {
        let state = self.state();
        let pool_full = state
            .store
            .as_ref()
            .is_some_and(crate::health::_helpers::pool::pool_full);
        cause.at(&state.target, pool_full)
    }

    pub fn control_succeeded(&self, at: DateTime<Utc>) {
        self.state().control_database.succeed(at);
    }

    /// Record a control database failure, from `request_id` when a request met it; returns since when it has been failing.
    pub fn control_failed(
        &self,
        cause: &ControlCause,
        (at, request_id): (DateTime<Utc>, Option<Uuid>),
    ) -> DateTime<Utc> {
        self.state().control_database.fail(ComponentFailure {
            at,
            since: at,
            reason: cause.message.clone(),
            case: Some(cause.case),
            chain: cause.chain.clone(),
            request_ids: request_id.into_iter().collect(),
        })
    }

    /// The control database's failure while it is failing, as a cause a response can carry.
    #[must_use]
    pub fn control_cause(&self) -> Option<ControlCause> {
        let state = self.state();
        let database = &state.control_database;
        let failure = database
            .last_failure
            .as_ref()
            .filter(|_| database.failing())?;
        Some(ControlCause {
            case: failure.case.unwrap_or(ControlCauseCase::Other),
            message: failure.reason.clone(),
            chain: failure.chain.clone(),
            transient: true,
            waited_for_pool: false,
        })
    }

    /// Since when the control database has been failing, if it is.
    #[must_use]
    pub fn control_failing_since(&self) -> Option<DateTime<Utc>> {
        let state = self.state();
        let database = &state.control_database;
        database
            .failing()
            .then(|| database.last_failure.as_ref().map(|failure| failure.since))
            .flatten()
    }

    pub fn scheduler_succeeded(&self, at: DateTime<Utc>) {
        if let Some(scheduler) = self.state().scheduler.as_mut() {
            scheduler.succeed(at);
        }
    }

    /// Record a failed scheduler cycle, with the control database's cause when it was the reason.
    pub fn scheduler_failed(
        &self,
        reason: &str,
        (at, cause): (DateTime<Utc>, Option<&ControlCause>),
    ) {
        if let Some(scheduler) = self.state().scheduler.as_mut() {
            scheduler.fail(ComponentFailure {
                at,
                since: at,
                reason: reason.to_owned(),
                case: cause.map(|cause| cause.case),
                chain: cause.map(|cause| cause.chain.clone()).unwrap_or_default(),
                request_ids: VecDeque::new(),
            });
        }
    }

    /// A location's instance reported at `at`.
    pub fn location_reported(&self, location_id: Uuid, at: DateTime<Utc>) {
        if let Some(location) = self.state().locations.get_mut(&location_id) {
            location.last_report = Some(location.last_report.map_or(at, |last| last.max(at)));
        }
    }

    /// The active locations as the control database has them, keeping any newer report seen here.
    pub(crate) fn locations_seen(&self, stored: Vec<StoredLocationReport>) {
        let mut state = self.state();
        let previous = std::mem::take(&mut state.locations);
        state.locations = stored
            .into_iter()
            .map(|location| {
                let seen = previous
                    .get(&location.location_id)
                    .and_then(|seen| seen.last_report);
                let last_report = seen.max(location.last_seen_at);
                (
                    location.location_id,
                    SeenLocation {
                        name: location.name,
                        last_report,
                    },
                )
            })
            .collect();
    }
}
