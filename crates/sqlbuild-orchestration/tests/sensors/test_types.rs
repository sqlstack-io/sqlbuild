use crate::runs::models::{NodeOutcome, RunOutcome, RunStatus};
use crate::sensors::models::SensorFunctionOutcome;
use crate::store::tests::store_test_types::Backend;

pub(crate) struct SensorTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_runs: usize,
}

pub(crate) struct CustomSensorResultCase {
    pub(crate) outcome: SensorFunctionOutcome,
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

pub(crate) struct CheckEventTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    /// The `check_event` kinds the sensor watches.
    pub(crate) kinds: &'static [&'static str],
    /// The command of an ad-hoc watched run; empty watches a job's run.
    pub(crate) adhoc_command: &'static [crate::runs::models::RunCommand],
    /// The checks whose outcome starts a run, each once.
    pub(crate) expected_checks: &'static [&'static str],
}

/// How many ticks in a row end the same way, and how they end.
pub(crate) type FailingTicks = (
    usize,
    fn() -> crate::sensors::models::SensorEvaluationOutcome,
);

pub(crate) struct SensorFailingTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    /// How the ticks after one good tick end, oldest first: how many in a row end each way.
    pub(crate) ticks: &'static [FailingTicks],
    pub(crate) expected_failed_ticks: Option<usize>,
    pub(crate) expected_reason: Option<&'static str>,
    /// Why the newest tick was abandoned, as people see it.
    pub(crate) expected_abandoned_because: Option<crate::sensors::models::AbandonReason>,
}

pub(crate) struct FunctionCursorTestCase {
    pub(crate) description: &'static str,
    /// Whether the tick is called with an event, as source and run sensors are.
    pub(crate) with_event: bool,
    pub(crate) expected_cursor: Option<&'static str>,
}

pub(crate) struct RunKeyTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    /// The `(job, run_key)` requests each tick returns.
    pub(crate) requests: &'static [(&'static str, &'static str)],
    pub(crate) expected_runs: usize,
}

pub(crate) struct SensorUpgradeTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    /// The runs of the job after one fails: it and its retry.
    pub(crate) expected_job_runs: usize,
}

pub(crate) struct SkipEventTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    /// The event the next tick is called with after the stuck one is skipped.
    pub(crate) expected_next_event: &'static str,
}

pub(crate) struct SensorPermissionTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    /// The member's permissions, and the nodes of the target they cover.
    pub(crate) permissions: &'static [&'static str],
    pub(crate) nodes: &'static [&'static str],
    /// Pause, reset-cursor and skip-event: applied, or why each was refused.
    pub(crate) expected_outcomes: [Result<bool, &'static str>; 3],
}

pub(crate) struct SkipInFlightTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    /// Operator changes before the skip, such as a pause, and after it.
    pub(crate) before_skip: &'static [crate::declarations::models::TriggerChange],
    pub(crate) after_skip: &'static [crate::declarations::models::TriggerChange],
    pub(crate) expected_next_event: &'static str,
}
