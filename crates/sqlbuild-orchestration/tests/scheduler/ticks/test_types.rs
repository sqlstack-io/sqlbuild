//! Database cases and expected scheduler tick outcomes.

pub(crate) use crate::store::tests::fault_types::TransactionFault;
use crate::store::tests::store_test_types::Backend;

pub(crate) struct TickTransactionTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_ticks: usize,
    pub(crate) expected_runs: usize,
}

pub(crate) struct TriggerControlsTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_events: usize,
}

pub(crate) struct IdleLauncher;

impl crate::scheduler::types::RunLauncher for IdleLauncher {
    async fn poll(
        &mut self,
        _store: &crate::store::models::ControlStore,
        _lease: Option<&crate::scheduler::models::SchedulerLease>,
    ) -> Result<(), crate::store::errors::ControlError> {
        Ok(())
    }

    async fn shutdown(
        &mut self,
        _store: &crate::store::models::ControlStore,
        _lease: Option<&crate::scheduler::models::SchedulerLease>,
    ) -> Result<(), crate::store::errors::ControlError> {
        Ok(())
    }
}

pub(crate) struct TickRetentionTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_survivors: usize,
}

pub(crate) struct LatestCatchUpTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) catch_up: serde_json::Value,
    pub(crate) expected_ticks: usize,
}

pub(crate) struct FirstTickPrecisionTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_first_tick_minutes_after_start: i64,
    pub(crate) expected_fractional_ticks: usize,
    pub(crate) expected_repeated_occurrences: usize,
}

/// Every way a randomized fault run can break the exactly-once contract; all zero when it holds.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct FaultAnomalies {
    pub(crate) duplicate_ticks: usize,
    pub(crate) missing_ticks: usize,
    pub(crate) off_schedule_ticks: usize,
    pub(crate) runs_without_exactly_one_tick: usize,
    pub(crate) request_event_mismatch: usize,
    pub(crate) unfenced_stale_writes: usize,
    pub(crate) accepted_duplicate_ticks: usize,
}

pub(crate) struct SchedulerFaultTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_anomalies: FaultAnomalies,
    pub(crate) expected_points_without_a_crash: usize,
}

/// One randomized step: who restarts, whether the lease lapses, which fault is armed, who cycles.
pub(crate) struct FaultStep {
    pub(crate) restart: Option<usize>,
    pub(crate) expire: bool,
    pub(crate) fault: usize,
    pub(crate) cycles: [bool; 2],
}

pub(crate) struct SchedulerLatencyTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_queued_ticks: usize,
    pub(crate) expected_ticks_after_failover: usize,
    pub(crate) expected_max_tick_to_queued: chrono::TimeDelta,
    pub(crate) expected_max_failover_catch_up: chrono::TimeDelta,
}

pub(crate) struct SchedulerUpgradeTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_follower_ticks: usize,
    pub(crate) expected_duplicate_minutes: usize,
    pub(crate) expected_missing_minutes: usize,
    pub(crate) expected_runs_per_tick: usize,
}
