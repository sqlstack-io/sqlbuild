//! Inputs make cursor, catch-up, and overlap decisions observably different.

use crate::scheduler::models::{CatchUp, Overlap};

#[derive(Debug)]
pub(super) struct TickTestCase {
    pub(super) description: &'static str,
    pub(super) enabled: bool,
    pub(super) due_seconds: Vec<i64>,
    pub(super) last_second: Option<i64>,
    pub(super) now_second: i64,
    pub(super) catch_up: CatchUp,
    pub(super) overlap: Overlap,
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
pub(super) struct BacklogTestCase {
    pub(super) description: &'static str,
    pub(super) catch_up: CatchUp,
    pub(super) expected_latest: i64,
    pub(super) expected_count: usize,
}

#[derive(Debug)]
pub(super) struct SimulationTestCase {
    pub(super) description: &'static str,
    pub(super) catch_up: CatchUp,
    pub(super) expected_success: bool,
}
