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
}

pub(crate) struct PinnedOldDocumentTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    /// The concurrency source an older release stored in the snapshot's jobs document.
    pub(crate) stored_source: &'static str,
    /// The `ControlError::Invalid` message advancing the group returns.
    pub(crate) expected_error: &'static str,
}
