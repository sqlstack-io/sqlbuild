use std::sync::{Arc, Mutex};

use crate::platform::models::{PlatformEvent, PlatformEventEnvelope, StorageKind};
use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::store::tests::store_test_types::Backend;

pub(crate) struct WatchdogTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) leaders: usize,
    pub(crate) expected_no_leader: bool,
    pub(crate) expected_overdue: usize,
    pub(crate) expected_low_horizons: usize,
}

pub(crate) struct LocationWatchdogTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_broken: usize,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Observation {
    pub(crate) storage_id: i64,
    pub(crate) occurred_at: DateTime<Utc>,
    pub(crate) payload: PlatformPayload,
}

#[derive(Debug, Deserialize)]
pub(crate) struct PlatformPayload {
    pub(crate) platform: PlatformEvent,
}

pub(crate) struct PlatformSinkTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) initial_status: u16,
    pub(crate) expected_events: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct WebhookReceiver {
    pub(crate) state: Arc<Mutex<WebhookState>>,
    pub(crate) delivered: tokio::sync::watch::Sender<usize>,
}

#[derive(Debug)]
pub(crate) struct WebhookState {
    pub(crate) status: u16,
    pub(crate) requests: Vec<PlatformEventEnvelope>,
}

pub(crate) struct ProblemsTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) leaders: usize,
    pub(crate) expected_leader: bool,
}

pub(crate) struct InstallationTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_storage: StorageKind,
}

pub(crate) struct CompileFailureTestCase {
    pub(crate) description: &'static str,
    pub(crate) finding_count: u64,
    pub(crate) listed: usize,
    pub(crate) message_chars: usize,
    pub(crate) expected_accepted: bool,
}

pub(crate) struct ProviderSecretsHealthTestCase {
    pub(crate) description: &'static str,
    /// Providers the target's `allow_missing_secrets` lists.
    pub(crate) allowed: &'static [&'static str],
    pub(crate) expected_providers: &'static [(&'static str, bool)],
    pub(crate) expected_missing: u64,
    pub(crate) expected_state: crate::platform::models::LocationState,
}

/// A location instance in a health history, by when its process started.
#[derive(Clone, Copy)]
pub(crate) enum Instance {
    /// Started three hours before the read.
    Previous,
    /// Started 25 seconds before the read.
    Restarted,
    /// Started eight days before the read, before the history kept.
    LongRunning,
    /// An instance whose session ID carries no start time.
    Unordered,
}

/// One report in a health history: the instance, how many seconds before the read, and what it said.
pub(crate) struct Reported {
    pub(crate) instance: Instance,
    pub(crate) seconds_ago: i64,
    pub(crate) health: ReportedHealth,
}

/// What a report in a health history said.
#[derive(Clone, Copy)]
pub(crate) enum ReportedHealth {
    /// The instance's liveness row.
    CheckIn,
    /// A publication of the generation, then its healthy connection check.
    Published(i64),
    /// A failed load of the generation.
    Failed(i64),
    /// A provider check without missing secrets.
    Secrets,
}

pub(crate) struct LocationStateTestCase {
    pub(crate) description: &'static str,
    pub(crate) reported: Vec<Reported>,
    /// Provider checks the previous instance sent after the restart, filling the history.
    pub(crate) later_reports: usize,
    pub(crate) expected_state: crate::platform::models::LocationState,
}
