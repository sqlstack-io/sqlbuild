use crate::store_test_types::Backend;

pub(crate) struct TickTransactionTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_ticks: usize,
    pub(crate) expected_runs: usize,
}

pub(crate) struct TickFaultTestCase {
    pub(crate) description: &'static str,
    pub(crate) table: &'static str,
    pub(crate) operation: &'static str,
    pub(crate) condition: &'static str,
    pub(crate) sqlite_body: &'static str,
    pub(crate) postgres_body: &'static str,
}

pub(crate) struct TriggerControlsTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_events: usize,
}
