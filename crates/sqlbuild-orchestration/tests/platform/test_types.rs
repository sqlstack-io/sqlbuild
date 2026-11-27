use std::sync::{Arc, Mutex};

use crate::platform::models::{LocationHealth, PlatformEvent, PlatformEventEnvelope, StorageKind};
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
    /// The load error's length.
    pub(crate) error_chars: usize,
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
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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

/// What an instance said: a report of the generation, built by a health helper such as `published`.
#[derive(Clone, Copy)]
pub(crate) struct ReportedHealth(pub(crate) fn(i64) -> LocationHealth, pub(crate) i64);

pub(crate) struct LocationStateTestCase {
    pub(crate) description: &'static str,
    pub(crate) reported: Vec<Reported>,
    /// Provider checks the previous instance sent after the restart, filling the history.
    pub(crate) later_reports: usize,
    pub(crate) expected_state: crate::platform::models::LocationState,
}

pub(crate) struct FailingLocationTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: crate::store::tests::store_test_types::Backend,
    pub(crate) expected_error: &'static str,
}

pub(crate) struct FailingSyncTestCase {
    pub(crate) description: &'static str,
    /// Newest first, as `recent_health` returns them.
    pub(crate) observations: Vec<crate::platform::models::LocationHealthObservation>,
    pub(crate) expected_sync: Option<crate::platform::models::FailingSync>,
}

pub(crate) struct FailingSinkTestCase {
    pub(crate) description: &'static str,
    /// Newest first, as the scan reads them.
    pub(crate) reports: Vec<crate::platform::models::SinkReport>,
    pub(crate) expected_sinks: Vec<crate::platform::models::FailingSink>,
}

pub(crate) struct StoredSinkTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: crate::store::tests::store_test_types::Backend,
    /// Each failing sink with its reason.
    pub(crate) expected_sinks: Vec<(String, String)>,
}

pub(crate) struct LocationSinkTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: crate::store::tests::store_test_types::Backend,
    /// Each reported interval's accepted, dropped and failed events, oldest first.
    pub(crate) intervals: Vec<(u64, u64, u64)>,
    /// Each failing sink with its reason and run.
    pub(crate) expected_sinks: Vec<(String, String, Option<uuid::Uuid>)>,
}

pub(crate) struct PlatformEventReadTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    /// The readable kinds the person sees, sorted.
    pub(crate) expected_kinds: [crate::platform::models::PlatformEventKind; 9],
    /// The stored type of the one event this server can't read.
    pub(crate) expected_unreadable_type: &'static str,
}

pub(crate) struct UnreadableEventTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    /// Which reader goes first: 0 for webhook delivery, 1 for the platform sensor.
    pub(crate) reader_rotation: usize,
    /// Runs the sensor starts: one per readable event of a kind it watches.
    pub(crate) expected_sensor_runs: usize,
    /// Events the webhook receives: every readable one stored, and the skip.
    pub(crate) expected_deliveries: usize,
    /// What the problem says is wrong with the skipped event.
    pub(crate) expected_reason: &'static str,
}

pub(crate) struct SkippedEventDismissalTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    /// The stored type of the skipped event the problem names.
    pub(crate) expected_type: &'static str,
}
