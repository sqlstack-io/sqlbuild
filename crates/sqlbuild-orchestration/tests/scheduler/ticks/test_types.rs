//! Database cases and expected scheduler tick outcomes.

pub(crate) use crate::fault_types::TransactionFault;
use crate::store_test_types::Backend;

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

impl sqlbuild_orchestration::scheduler::types::RunLauncher for IdleLauncher {
    async fn poll(
        &mut self,
        _store: &sqlbuild_orchestration::store::models::ControlStore,
        _lease: Option<&sqlbuild_orchestration::scheduler::models::SchedulerLease>,
    ) -> Result<(), sqlbuild_orchestration::store::errors::ControlError> {
        Ok(())
    }

    async fn shutdown(
        &mut self,
        _store: &sqlbuild_orchestration::store::models::ControlStore,
        _lease: Option<&sqlbuild_orchestration::scheduler::models::SchedulerLease>,
    ) -> Result<(), sqlbuild_orchestration::store::errors::ControlError> {
        Ok(())
    }
}
