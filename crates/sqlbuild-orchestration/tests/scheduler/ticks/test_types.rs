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
