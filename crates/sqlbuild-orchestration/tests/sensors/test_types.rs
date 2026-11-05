use crate::runs::models::{NodeOutcome, RunOutcome};
use crate::sensors::models::CustomSensorOutcome;
use crate::store::tests::store_test_types::Backend;

pub(crate) struct SensorTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_runs: usize,
}

pub(crate) struct CustomSensorResultCase {
    pub(crate) outcome: CustomSensorOutcome,
    pub(crate) expected_phase: &'static str,
    pub(crate) expected_cursor: Option<&'static str>,
}

pub(crate) struct SensorExecutionTestCase {
    pub(crate) description: &'static str,
    pub(crate) outcome: RunOutcome,
    pub(crate) nodes: [NodeOutcome; 2],
    pub(crate) started: [bool; 2],
    pub(crate) expected_selector: &'static str,
    pub(crate) expected_filters: &'static [&'static str],
}
