use crate::runs::models::{NodeOutcome, RunOutcome, RunStatus};
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

pub(crate) struct GroupMemberSensorTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    /// The node and run outcome the first member ends with.
    pub(crate) staging: (NodeOutcome, RunOutcome),
    /// The node and run outcome the dependent member ends with.
    pub(crate) marts: (NodeOutcome, RunOutcome),
    /// Sensors that fire once the first member ends, while the group still runs.
    pub(crate) expected_firing: &'static [&'static str],
    /// Member sensors that have fired once the dependent member ends too.
    pub(crate) expected_after_marts: &'static [&'static str],
}

pub(crate) struct SkippedMemberSensorTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_marts_status: RunStatus,
    /// Sensors that fire, with how many runs each requested.
    pub(crate) expected_fired: &'static [(&'static str, usize)],
}
