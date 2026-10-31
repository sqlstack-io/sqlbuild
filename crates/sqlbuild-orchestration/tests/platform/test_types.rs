use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlbuild_orchestration::platform::models::PlatformEvent;

use crate::store_test_types::Backend;

pub(crate) struct WatchdogTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) leaders: usize,
    pub(crate) expected_no_leader: bool,
    pub(crate) expected_overdue: usize,
    pub(crate) expected_low_horizons: usize,
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
