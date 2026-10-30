pub(crate) use crate::fault_types::TickFaultTestCase;
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
