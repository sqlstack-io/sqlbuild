//! Clock-independent schedule fixtures.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use proptest::prelude::any;
use proptest::test_runner::{Config, TestCaseError, TestCaseResult, TestRunner};
use proptest::{prop_assert, prop_assert_eq};
use uuid::Uuid;

use crate::scheduler::main::_plan_ticks::plan_ticks;
use crate::scheduler::models::{CatchUp, Overlap, TickAction, TickDecision, TickPlan, TickState};
use crate::scheduler::tests::test_types::TickTestCase;

pub(super) fn time(second: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(second, 0).expect("synthetic schedule time is representable")
}

pub(super) fn state(test_case: &TickTestCase) -> TickState {
    TickState {
        trigger_id: Uuid::from_u128(19),
        enabled: test_case.enabled,
        last_tick_for: test_case.last_second.map(time),
        due_times: test_case
            .due_seconds
            .iter()
            .map(|second| time(*second))
            .collect(),
        catch_up: test_case.catch_up,
        overlap: test_case.overlap,
        waiting_run: test_case.active.then_some(Uuid::from_u128(31)),
        continuation: None,
    }
}

pub(super) fn request_seconds(plan: &TickPlan) -> Vec<i64> {
    plan.actions
        .iter()
        .filter(|action| matches!(action.decision, TickDecision::Requested { .. }))
        .map(|action| action.scheduled_for.timestamp())
        .collect()
}

pub(super) fn covered_seconds(plan: &TickPlan) -> Vec<i64> {
    plan.actions
        .iter()
        .filter(|action| matches!(action.decision, TickDecision::Covered { .. }))
        .map(|action| action.scheduled_for.timestamp())
        .collect()
}

fn exercise_history(due: Vec<i64>, history: Vec<(i64, bool)>, catch_up: CatchUp) -> TestCaseResult {
    let expected: BTreeSet<i64> = due.iter().copied().collect();
    let mut state = TickState {
        trigger_id: Uuid::from_u128(19),
        enabled: true,
        last_tick_for: None,
        due_times: due.into_iter().map(time).collect(),
        catch_up,
        overlap: Overlap::Queue,
        waiting_run: None,
        continuation: None,
    };
    let mut committed: BTreeMap<i64, TickAction> = BTreeMap::new();
    for (clock, commit_succeeds) in history {
        let plan = plan_ticks(&state, time(clock))
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        let Some(plan) = commit_succeeds.then_some(plan) else {
            continue;
        };
        for action in plan.actions {
            prop_assert!(action.scheduled_for.timestamp() <= clock);
            prop_assert!(
                committed
                    .insert(action.scheduled_for.timestamp(), action)
                    .is_none(),
                "a committed tick was emitted twice"
            );
        }
        state.last_tick_for = plan.last_tick_for;
    }
    for _ in 0..=expected.len() {
        let plan = plan_ticks(&state, time(1001))
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        let Some(_) = plan.actions.first() else {
            break;
        };
        for action in plan.actions {
            prop_assert!(
                committed
                    .insert(action.scheduled_for.timestamp(), action)
                    .is_none(),
                "restart repeated a committed tick"
            );
        }
        state.last_tick_for = plan.last_tick_for;
    }
    prop_assert_eq!(
        committed.keys().copied().collect::<BTreeSet<_>>(),
        expected,
        "a due tick was lost or invented"
    );
    prop_assert_eq!(
        committed
            .values()
            .map(|action| action.tick_id)
            .collect::<BTreeSet<_>>()
            .len(),
        committed.len()
    );
    Ok(())
}

pub(super) fn simulate(catch_up: CatchUp) -> Result<(), String> {
    let strategy = (
        proptest::collection::vec(0_i64..1000, 0..128),
        proptest::collection::vec((-100_i64..1100, any::<bool>()), 0..128),
    );
    let mut property_runner = TestRunner::new(Config {
        cases: 512,
        failure_persistence: None,
        ..Config::default()
    });
    property_runner
        .run(&strategy, |(due, history)| {
            exercise_history(due, history, catch_up)
        })
        .map_err(|error| error.to_string())
}
