use std::sync::{Arc, Mutex};

use crate::platform::models::{PlatformEvent, PlatformEventEnvelope};
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
