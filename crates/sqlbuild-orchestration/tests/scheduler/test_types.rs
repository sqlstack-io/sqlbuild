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
    /// The scheduler cycle after 0004, before the location publishes again: `ok` when it carries on.
    pub(crate) expected_cycle: &'static str,
    /// Ticks that cycle takes, from the other location's schedule.
    pub(crate) expected_ticks: usize,
    /// The triggers the live problems list as skipped meanwhile, as `name: reason`.
    pub(crate) expected_skipped: &'static [&'static str],
    /// Ticks the next cycle takes once the location has published again.
    pub(crate) expected_ticks_after_republish: usize,
}

pub(crate) struct PinnedOldDocumentTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    /// The concurrency source an older release stored in the snapshot's jobs document.
    pub(crate) stored_source: &'static str,
    /// Advancing the location's groups: `ok` when it carries on.
    pub(crate) expected_advance: &'static str,
    /// Every run afterwards, as `job status: reason`.
    pub(crate) expected_runs: &'static [&'static str],
}

pub(crate) struct FailedJobRunOldDocumentTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    /// The concurrency source an older release stored with each job.
    pub(crate) stored_source: &'static str,
    /// The outcome of each worker report: started, the node finished, the run failed.
    pub(crate) expected_reports: [&'static str; 3],
    /// Every run afterwards, as `job status: reason`.
    pub(crate) expected_runs: &'static [&'static str],
}

pub(crate) struct ClaimOldDocumentTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    /// The concurrency source an older release stored in the snapshot's documents.
    pub(crate) stored_source: &'static str,
    /// The claimed run, as `location job`.
    pub(crate) expected_claimed: &'static str,
    /// The group member whose snapshot can't be read, as `job status: reason`.
    pub(crate) expected_member: &'static str,
}

pub(crate) struct AdhocOldDocumentTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    /// The ad hoc plan check's outcome.
    pub(crate) expected_check: &'static str,
}

pub(crate) struct UnreadableSensorTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    /// What an earlier release left behind, applied after the sensor's first check.
    pub(crate) earlier_form: &'static str,
    /// The second scheduler cycle: `ok` when it carries on.
    pub(crate) expected_cycle: &'static str,
    /// The triggers the live problems list as skipped afterwards, as `name: reason`.
    pub(crate) expected_skipped: &'static [&'static str],
    /// Runs the sensor has queued by then.
    pub(crate) expected_sensor_runs: i64,
}

pub(crate) struct RunningMemberOldDocumentTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    /// The concurrency source an older release stored in the snapshot's jobs document.
    pub(crate) stored_source: &'static str,
    /// Every run after the pass that meets the unreadable snapshot, as `job status: reason`.
    pub(crate) expected_while_running: &'static [&'static str],
    /// The runs asked to cancel by then.
    pub(crate) expected_cancel_requested: &'static [&'static str],
    /// Every run after the running member stops and the next pass, as `job status: reason`.
    pub(crate) expected_after: &'static [&'static str],
}
