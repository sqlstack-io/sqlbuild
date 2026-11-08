//! Table rows for the ad-hoc planner.

/// A command, and the exact keys and settings it plans to, or a fragment of its refusal.
pub(super) struct AdhocPlanTestCase {
    pub(super) description: &'static str,
    pub(super) command: &'static [&'static str],
    pub(super) expected_plan: Result<(&'static [&'static str], serde_json::Value), &'static str>,
}
