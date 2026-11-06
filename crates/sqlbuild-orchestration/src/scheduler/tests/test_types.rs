//! Inputs make cursor, catch-up, and coalescing decisions observably different.

use crate::store::tests::store_test_types::Backend;

pub(super) struct SensorFairnessTestCase {
    pub(super) description: &'static str,
    pub(super) backend: Backend,
    pub(super) pending: usize,
    pub(super) builtin_status: &'static str,
    pub(super) expected_completed: usize,
}

#[derive(Debug)]
pub(super) struct TickTestCase {
    pub(super) description: &'static str,
    pub(super) due_seconds: Vec<i64>,
    pub(super) last_second: Option<i64>,
    pub(super) now_second: i64,
    pub(super) active: bool,
    pub(super) expected_requests: Vec<i64>,
    pub(super) expected_covered: Vec<i64>,
    pub(super) expected_last: Option<i64>,
    pub(super) expected_next: Option<i64>,
}

#[derive(Debug)]
pub(super) struct CronTestCase {
    pub(super) description: &'static str,
    pub(super) expression: &'static str,
    pub(super) timezone: &'static str,
    pub(super) after: &'static str,
    pub(super) expected_next: &'static str,
}

#[derive(Debug)]
pub(super) struct PreviousCronTestCase {
    pub(super) description: &'static str,
    pub(super) expression: &'static str,
    pub(super) timezone: &'static str,
    pub(super) at: &'static str,
    pub(super) expected_previous: &'static str,
}

#[derive(Debug)]
pub(super) struct SimulationTestCase {
    pub(super) description: &'static str,
    pub(super) expected_success: bool,
}

pub(super) struct CronSimulationTestCase {
    pub(super) description: &'static str,
    pub(super) expression: &'static str,
    pub(super) wall_times: &'static [(u32, u32)],
    pub(super) expected_success: bool,
}
