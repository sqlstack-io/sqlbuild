//! Leadership scenarios run independently on both supported engines.

use crate::store::tests::store_test_types::Backend;

#[derive(Debug)]
pub(crate) struct LeadershipTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_epoch: i64,
}

pub(crate) struct MigratedConcurrencySourceTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    /// The concurrency source an older release stored in each job's definition.
    pub(crate) stored_source: &'static str,
    /// Whether a manual run of the job is refused before migration 0004.
    pub(crate) expected_refused_before: bool,
    pub(crate) expected_resolved_concurrency: serde_json::Value,
    pub(crate) expected_ticks: usize,
    /// Runs queued after 0004, as `origin count`.
    pub(crate) expected_runs: &'static [&'static str],
    /// A job page read and a group run request after 0004, before the location publishes again.
    pub(crate) expected_until_republished: [&'static str; 2],
    /// The same once the location has published again.
    pub(crate) expected_after_republish: [&'static str; 2],
}

pub(crate) struct GroupScheduleOldDocumentTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    /// The concurrency source an older release stored with each job.
    pub(crate) stored_source: &'static str,
    /// The scheduler cycle due to queue the group after 0004, before the location publishes again.
    pub(crate) expected_until_republished: &'static str,
    /// Ticks the cycle takes once the location has published again.
    pub(crate) expected_ticks_after_republish: usize,
}

pub(crate) struct PinnedOldDocumentTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    /// The concurrency source an older release stored in the snapshot's jobs document.
    pub(crate) stored_source: &'static str,
    /// The `ControlError::Invalid` message advancing the group returns.
    pub(crate) expected_error: &'static str,
}
