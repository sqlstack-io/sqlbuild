//! Table cases of the warehouse state a location builds.

use serde_json::Value;

/// A state's cursor `MAX` of orders, newest shop fingerprint runs, consumption row count and version.
pub(crate) type StateAnswers = (Option<String>, Vec<String>, usize, Option<String>);

/// The locations a sync asks to resume in, and the adapter and pause table it resumes for.
pub(crate) struct ResumeLocationsTestCase {
    pub(crate) description: &'static str,
    pub(crate) requested: &'static [(&'static str, Option<&'static str>)],
    pub(crate) dynamic: bool,
    pub(crate) pause_schema: Option<&'static str>,
    /// Each location's schema and quiet token, in order.
    pub(crate) expected_locations: &'static [(&'static str, Option<&'static str>)],
}

/// A served copy and a stored fixture of the same facts, and what both states answer.
pub(crate) struct PreviewStateTestCase {
    pub(crate) description: &'static str,
    pub(crate) served: fn() -> Value,
    pub(crate) stored: fn() -> Value,
    pub(crate) expected_answers: fn() -> StateAnswers,
}
