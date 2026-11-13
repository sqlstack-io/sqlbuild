//! Clock-independent schedule fixtures.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, TimeDelta, Utc};
use proptest::prelude::any;
use proptest::test_runner::{Config, TestCaseError, TestCaseResult, TestRunner};
use proptest::{prop_assert, prop_assert_eq};
use uuid::Uuid;

use crate::declarations::models::{Schedule, TriggerDefinition, TriggerStatus};
use crate::scheduler::main::_plan_ticks::plan_ticks;
use crate::scheduler::models::{TickAction, TickDecision, TickPlan, TickState};
use crate::scheduler::tests::test_types::TickTestCase;

pub(super) fn time(second: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(second, 0).expect("synthetic schedule time is representable")
}

pub(super) fn state(test_case: &TickTestCase) -> (TriggerDefinition, TickState) {
    (
        calendar(test_case.due_seconds.iter().copied()),
        TickState {
            trigger_id: Uuid::from_u128(19),
            started_at: time(0),
            last_tick_for: test_case.last_second.map(time),
            waiting_run: test_case.active.then_some(Uuid::from_u128(31)),
            active_run: test_case.running.then_some(Uuid::from_u128(41)),
        },
    )
}

fn calendar(seconds: impl Iterator<Item = i64>) -> TriggerDefinition {
    TriggerDefinition {
        description: "orders calendar".into(),
        job_key: "orders".into(),
        starts_at: Some(time(0)),
        default_status: TriggerStatus::Running,
        targets: BTreeSet::new(),
        declared_at: None,
        schedule: Schedule::Calendar {
            times: seconds.map(time).collect(),
            refresh_at: time(2000),
        },
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

pub(super) fn skipped_seconds(plan: &TickPlan) -> Vec<i64> {
    plan.actions
        .iter()
        .filter(|action| matches!(action.decision, TickDecision::Skipped { .. }))
        .map(|action| action.scheduled_for.timestamp())
        .collect()
}

/// Under crashes and clock jumps a tick is never early, repeated or invented, and the latest due time always ticks.
fn exercise_history(due: Vec<i64>, history: Vec<(i64, bool)>) -> TestCaseResult {
    let expected: BTreeSet<i64> = due.iter().copied().collect();
    let trigger = calendar(due.into_iter());
    let mut state = TickState {
        trigger_id: Uuid::from_u128(19),
        started_at: time(0),
        last_tick_for: None,
        waiting_run: None,
        active_run: None,
    };
    let mut committed: BTreeMap<i64, TickAction> = BTreeMap::new();
    let final_step = std::iter::once((1001, true));
    for (clock, commit_succeeds) in history.into_iter().chain(final_step) {
        let plan = plan_ticks(&trigger, &state, time(clock))
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
    let ticked: BTreeSet<i64> = committed.keys().copied().collect();
    prop_assert!(ticked.is_subset(&expected), "a tick was invented");
    prop_assert_eq!(
        ticked.last(),
        expected.last(),
        "the latest due time was lost"
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
    use chrono::TimeZone;
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

/// Real cron planning from a start time; each committed tick is due, distinct and later than the last.
fn cron_ticks(
    trigger: &TriggerDefinition,
    (start, end): (DateTime<Utc>, DateTime<Utc>),
    steps: impl Iterator<Item = (TimeDelta, bool)>,
) -> Result<BTreeSet<DateTime<Utc>>, TestCaseError> {
    let mut state = TickState {
        trigger_id: Uuid::from_u128(23),
        started_at: start,
        last_tick_for: None,
        waiting_run: None,
        active_run: None,
    };
    let mut committed: BTreeSet<DateTime<Utc>> = BTreeSet::new();
    let mut clock = start;
    for (jump, commit) in steps.chain(std::iter::once((end - start, true))) {
        clock = (clock + jump).clamp(start, end);
        let plan = plan_ticks(trigger, &state, clock)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        for action in plan.actions.iter().filter(|_| commit) {
            prop_assert!(
                action.scheduled_for <= clock,
                "a tick ran before it was due"
            );
            prop_assert!(
                committed
                    .last()
                    .is_none_or(|last| action.scheduled_for > *last),
                "a committed tick was emitted twice or out of order"
            );
            committed.insert(action.scheduled_for);
        }
        state.last_tick_for = [state.last_tick_for, plan.last_tick_for][usize::from(commit)];
    }
    Ok(committed)
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
        targets: BTreeSet::new(),
        declared_at: None,
    };
    let strategy = proptest::collection::vec((-1_440_i64..5_760, any::<bool>()), 0..256);
    let mut property_runner = TestRunner::new(Config {
        cases: 64,
        failure_persistence: None,
        ..Config::default()
    });
    let reference = reference_occurrences(chrono_tz::America::New_York, wall_times, (start, end));
    let every_step = std::iter::repeat_n((TimeDelta::minutes(10), true), 37_500);
    let stepped =
        cron_ticks(&trigger, (start, end), every_step).map_err(|error| error.to_string())?;
    (stepped == reference)
        .then_some(())
        .ok_or("ten-minute evaluation lost, repeated or invented an occurrence")?;
    property_runner
        .run(&strategy, |history| {
            let steps = history
                .into_iter()
                .map(|(minutes, commit)| (TimeDelta::minutes(minutes), commit));
            let ticked = cron_ticks(&trigger, (start, end), steps)?;
            prop_assert!(ticked.is_subset(&reference), "a tick was invented");
            prop_assert_eq!(
                ticked.last(),
                reference.last(),
                "the latest due time was lost"
            );
            Ok(())
        })
        .map_err(|error| error.to_string())
}
