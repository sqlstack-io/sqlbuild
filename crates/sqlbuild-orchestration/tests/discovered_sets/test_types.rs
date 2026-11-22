//! Test case types for discovered sets.

use crate::discovered_sets::models::{DiscoveredResult, DiscoveredSetStatus};

/// A discover function's result and the problem it has, if any.
pub(crate) struct ProblemTestCase {
    pub(crate) description: &'static str,
    pub(crate) result: DiscoveredResult,
    pub(crate) expected_problem: Option<String>,
}

/// A `check_every` text and its seconds, or `None` when it is refused.
pub(crate) struct CheckEveryTestCase {
    pub(crate) description: &'static str,
    pub(crate) text: &'static str,
    pub(crate) expected_seconds: Option<u64>,
}

/// An attempt after an earlier success, and what the set keeps.
pub(crate) struct AttemptTestCase {
    pub(crate) description: &'static str,
    pub(crate) attempt: Result<DiscoveredResult, String>,
    pub(crate) expected_ids: Vec<&'static str>,
    pub(crate) expected_status: DiscoveredSetStatus,
    pub(crate) expected_error: Option<&'static str>,
}

/// A set, its roots, and whether reordering its tags keeps its digest.
pub(crate) struct RootsTestCase {
    pub(crate) description: &'static str,
    pub(crate) result: DiscoveredResult,
    pub(crate) expected_roots: Vec<&'static str>,
    pub(crate) expected_same_digest: bool,
}

/// An attempt after an earlier success, the summary `sqb dev` shows, and whether it changed.
pub(crate) struct SummaryTestCase {
    pub(crate) description: &'static str,
    pub(crate) attempt: Result<DiscoveredResult, String>,
    pub(crate) expected_summary: &'static str,
    pub(crate) expected_changed: bool,
}

/// A first attempt and what the location's health reports of it.
pub(crate) struct HealthTestCase {
    pub(crate) description: &'static str,
    pub(crate) attempt: Result<DiscoveredResult, String>,
    pub(crate) expected_health: (DiscoveredSetStatus, u64, Option<&'static str>),
    pub(crate) expected_summary: &'static str,
}
