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

/// A cycle's phase timings and when the launcher poll after it finished, freeing the loop to sleep.
#[derive(Clone, Debug)]
pub(crate) struct RecordedCycle {
    pub(crate) timings: crate::scheduler::models::CycleTimings,
    pub(crate) polled_at: chrono::DateTime<chrono::Utc>,
}

/// Records cycle timings; polls take `poll_delay` after a cycle that decided a trigger, as launches do.
pub(crate) struct CycleRecorder<'a> {
    pub(crate) cycles: &'a mut Vec<RecordedCycle>,
    pub(crate) poll_delay: std::time::Duration,
    pub(crate) launching: bool,
}

impl crate::scheduler::types::RunLauncher for CycleRecorder<'_> {
    async fn poll(
        &mut self,
        _store: &crate::store::models::ControlStore,
        _lease: Option<&crate::scheduler::models::SchedulerLease>,
    ) -> Result<(), crate::store::errors::ControlError> {
        if self.launching {
            tokio::time::sleep(self.poll_delay).await;
        }
        if let Some(cycle) = self.cycles.last_mut() {
            cycle.polled_at = chrono::Utc::now();
        }
        Ok(())
    }

    async fn shutdown(
        &mut self,
        _store: &crate::store::models::ControlStore,
        _lease: Option<&crate::scheduler::models::SchedulerLease>,
    ) -> Result<(), crate::store::errors::ControlError> {
        Ok(())
    }

    fn cycled(&mut self, cycle: &crate::scheduler::models::SchedulerCycle) {
        self.cycles.push(RecordedCycle {
            timings: cycle.timings.clone(),
            polled_at: cycle.timings.started_at,
        });
        self.launching = cycle.evaluated > 0;
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
    pub(crate) expected_ticks: usize,
}

pub(crate) struct FirstTickPrecisionTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_first_tick_minutes_after_start: i64,
    pub(crate) expected_fractional_ticks: usize,
    pub(crate) expected_repeated_occurrences: usize,
}

/// Ways a fault run breaks exactly-once; times passed during an outage are skipped by design.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct FaultAnomalies {
    pub(crate) duplicate_ticks: usize,
    pub(crate) missing_latest_ticks: usize,
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
    pub(crate) expected_ticks: usize,
    pub(crate) expected_ticks_after_failover: usize,
    pub(crate) expected_max_wake_after_due: chrono::TimeDelta,
    pub(crate) expected_max_failover_wake: chrono::TimeDelta,
    /// Work after the cycle's next-due read, which must not delay the wake.
    pub(crate) launcher_poll: std::time::Duration,
}
