//! Clock-independent schedule fixtures.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use proptest::prelude::any;
use proptest::test_runner::{Config, TestCaseError, TestCaseResult, TestRunner};
use proptest::{prop_assert, prop_assert_eq};
use uuid::Uuid;

use crate::declarations::models::{Schedule, TriggerDefinition, TriggerStatus};
use crate::scheduler::_helpers::expansion::expand_schedule;
use crate::scheduler::main::_plan_ticks::plan_ticks;
use crate::scheduler::models::{TickAction, TickDecision, TickPlan, TickState};
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

fn exercise_history(due: Vec<i64>, history: Vec<(i64, bool)>) -> TestCaseResult {
    let expected: BTreeSet<i64> = due.iter().copied().collect();
    let mut state = TickState {
        trigger_id: Uuid::from_u128(19),
        enabled: true,
        last_tick_for: None,
        due_times: due.into_iter().map(time).collect(),
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

pub(super) fn simulate() -> Result<(), String> {
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
        .run(&strategy, |(due, history)| exercise_history(due, history))
        .map_err(|error| error.to_string())
}

/// Reference occurrences found day by day from local wall times, independent of cursor search.
fn reference_occurrences(
    timezone: chrono_tz::Tz,
    wall_times: &[(u32, u32)],
    (start, end): (DateTime<Utc>, DateTime<Utc>),
) -> BTreeSet<DateTime<Utc>> {
    use chrono::{TimeDelta, TimeZone};
    let first_date = start.with_timezone(&timezone).date_naive() - TimeDelta::days(1);
    let days = (end.with_timezone(&timezone).date_naive() - first_date).num_days();
    (0..=days)
        .flat_map(|day| {
            wall_times.iter().map(move |(hour, minute)| {
                let wall = (first_date + TimeDelta::days(day))
                    .and_hms_opt(*hour, *minute, 0)
                    .expect("wall time");
                // The later fold for repeated times; the first existing minute after a gap.
                (0..=180)
                    .find_map(|offset| {
                        timezone
                            .from_local_datetime(&(wall + TimeDelta::minutes(offset)))
                            .latest()
                    })
                    .expect("a local time exists within three hours")
                    .to_utc()
            })
        })
        .filter(|instant| *instant > start && *instant <= end)
        .collect()
}

/// Real cron expansion and planning under random crashes and clock jumps, including backwards.
fn exercise_cron_history(
    (trigger, timezone): (&TriggerDefinition, chrono_tz::Tz),
    wall_times: &[(u32, u32)],
    (start, end): (DateTime<Utc>, DateTime<Utc>),
    history: Vec<(i64, bool)>,
) -> TestCaseResult {
    use chrono::TimeDelta;
    let mut last_tick_for: Option<DateTime<Utc>> = None;
    let mut committed: BTreeSet<DateTime<Utc>> = BTreeSet::new();
    let mut clock = start;
    // After the random history, enough uncrashed steps at the end time to drain a bounded backlog.
    let steps = history
        .into_iter()
        .map(|(minutes, commit)| (TimeDelta::minutes(minutes), commit))
        .chain(std::iter::repeat_n((end - start, true), 500));
    for (jump, commit) in steps {
        clock = (clock + jump).clamp(start, end);
        let (due_times, continuation) = expand_schedule(
            trigger,
            last_tick_for.unwrap_or(start - TimeDelta::microseconds(1)),
            clock,
        )
        .map_err(|error| TestCaseError::fail(error.to_string()))?;
        let plan = plan_ticks(
            &TickState {
                trigger_id: Uuid::from_u128(23),
                enabled: true,
                last_tick_for,
                due_times,
                continuation,
                waiting_run: None,
            },
            clock,
        )
        .map_err(|error| TestCaseError::fail(error.to_string()))?;
        for action in plan.actions.iter().filter(|_| commit) {
            prop_assert!(
                action.scheduled_for <= clock,
                "a tick ran before it was due"
            );
            prop_assert!(
                committed.insert(action.scheduled_for),
                "a committed tick was emitted twice"
            );
        }
        last_tick_for = [last_tick_for, plan.last_tick_for][usize::from(commit)];
    }
    prop_assert_eq!(
        committed,
        reference_occurrences(timezone, wall_times, (start, end)),
        "a due tick was lost or invented"
    );
    Ok(())
}

/// Months spanning both 2026 New York transitions; clocks jump up to a day back or four days on.
pub(super) fn simulate_cron(expression: &str, wall_times: &[(u32, u32)]) -> Result<(), String> {
    let start = DateTime::parse_from_rfc3339("2026-02-25T00:00:00.25Z")
        .expect("start")
        .to_utc();
    let end = DateTime::parse_from_rfc3339("2026-11-10T00:00:00Z")
        .expect("end")
        .to_utc();
    let trigger = TriggerDefinition {
        description: "Simulated orders".into(),
        job_key: "orders".into(),
        schedule: Schedule::Cron {
            expression: expression.into(),
            timezone: "America/New_York".into(),
        },
        starts_at: Some(start),
        default_status: TriggerStatus::default(),
    };
    let strategy = proptest::collection::vec((-1_440_i64..5_760, any::<bool>()), 0..256);
    let mut property_runner = TestRunner::new(Config {
        cases: 64,
        failure_persistence: None,
        ..Config::default()
    });
    property_runner
        .run(&strategy, |history| {
            exercise_cron_history(
                (&trigger, chrono_tz::America::New_York),
                wall_times,
                (start, end),
                history,
            )
        })
        .map_err(|error| error.to_string())
}
