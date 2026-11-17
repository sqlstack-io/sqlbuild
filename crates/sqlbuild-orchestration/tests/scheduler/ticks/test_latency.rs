//! Due-time selection steps a set database clock; one real-clock run only catches a scheduler that hangs.

use std::time::Duration;

use chrono::{DurationRound, TimeDelta, Utc};

use crate::credentials::tests::location_helpers::create_location;
use crate::scheduler::main::list_ticks::list_ticks;
use crate::scheduler::tests::contracts::ticks::helpers::{
    publish_calendar, run_scheduler_until, step_cycles,
};
use crate::scheduler::tests::contracts::ticks::test_types::{
    ClockCycleStep, SchedulerClockTestCase, SchedulerWakeTestCase,
};
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::clock_helpers::with_database_clock;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

const ON_TIME_THEN_FAILOVER: &[ClockCycleStep] = &[
    ClockCycleStep {
        holder: 0,
        at_seconds: 0,
        expected_leader: true,
        expected_ticks: 0,
        expected_wait: Duration::from_secs(2),
    },
    ClockCycleStep {
        holder: 0,
        at_seconds: 2,
        expected_leader: true,
        expected_ticks: 1,
        expected_wait: Duration::from_secs(1),
    },
    ClockCycleStep {
        holder: 0,
        at_seconds: 3,
        expected_leader: true,
        expected_ticks: 1,
        expected_wait: Duration::from_secs(1),
    },
    ClockCycleStep {
        holder: 0,
        at_seconds: 4,
        expected_leader: true,
        expected_ticks: 1,
        expected_wait: Duration::from_secs(4),
    },
    // The leader stops without releasing its lease, renewed at +4 for 30 s, while +8 and +9 fall due.
    ClockCycleStep {
        holder: 1,
        at_seconds: 10,
        expected_leader: false,
        expected_ticks: 0,
        expected_wait: Duration::from_secs(5),
    },
    ClockCycleStep {
        holder: 1,
        at_seconds: 33,
        expected_leader: false,
        expected_ticks: 0,
        expected_wait: Duration::from_secs(5),
    },
    // The standby takes over once the lease lapses and ticks only the latest overdue time.
    ClockCycleStep {
        holder: 1,
        at_seconds: 35,
        expected_leader: true,
        expected_ticks: 1,
        expected_wait: Duration::from_secs(5),
    },
];

const LATE_CYCLE: &[ClockCycleStep] = &[
    ClockCycleStep {
        holder: 0,
        at_seconds: 0,
        expected_leader: true,
        expected_ticks: 0,
        expected_wait: Duration::from_secs(2),
    },
    ClockCycleStep {
        holder: 0,
        at_seconds: 1,
        expected_leader: true,
        expected_ticks: 0,
        expected_wait: Duration::from_secs(1),
    },
    // +2 and +3 are both overdue: one tick, for +3.
    ClockCycleStep {
        holder: 0,
        at_seconds: 3,
        expected_leader: true,
        expected_ticks: 1,
        expected_wait: Duration::from_secs(1),
    },
    ClockCycleStep {
        holder: 0,
        at_seconds: 4,
        expected_leader: true,
        expected_ticks: 1,
        expected_wait: Duration::from_secs(5),
    },
];

#[test]
fn given_a_set_database_clock_when_times_fall_due_then_each_cycle_ticks_the_latest_due_time_and_sleeps_until_the_next()
 {
    let test_cases = [
        SchedulerClockTestCase {
            description: "SQLite on-time ticks then standby catch-up",
            backend: Backend::Sqlite,
            calendar: &[2, 3, 4, 8, 9],
            steps: ON_TIME_THEN_FAILOVER,
            expected_ticked: &[2, 3, 4, 9],
        },
        SchedulerClockTestCase {
            description: "PostgreSQL on-time ticks then standby catch-up",
            backend: Backend::Postgres,
            calendar: &[2, 3, 4, 8, 9],
            steps: ON_TIME_THEN_FAILOVER,
            expected_ticked: &[2, 3, 4, 9],
        },
        SchedulerClockTestCase {
            description: "SQLite late cycle folds overdue times",
            backend: Backend::Sqlite,
            calendar: &[2, 3, 4],
            steps: LATE_CYCLE,
            expected_ticked: &[3, 4],
        },
        SchedulerClockTestCase {
            description: "PostgreSQL late cycle folds overdue times",
            backend: Backend::Postgres,
            calendar: &[2, 3, 4],
            steps: LATE_CYCLE,
            expected_ticked: &[3, 4],
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            let base = Utc::now()
                .duration_trunc(TimeDelta::seconds(1))
                .expect("second");
            let ticks = with_database_clock(base, async {
                migrate_store(store).await.expect("migrate");
                let location = create_location(store, "orders", "test-admin")
                    .await
                    .expect("location");
                let trigger_id =
                    publish_calendar(store, location, (base, test_case.calendar)).await;
                step_cycles(store, (base, test_case.steps, test_case.description)).await;
                list_ticks(store, trigger_id, 100).await.expect("ticks")
            })
            .await;
            let mut ticked: Vec<i64> = ticks
                .iter()
                .map(|tick| (tick.scheduled_for - base).num_seconds())
                .collect();
            ticked.sort_unstable();
            assert_eq!(
                ticked, test_case.expected_ticked,
                "{}",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_a_running_scheduler_when_a_time_falls_due_then_it_is_decided_without_hanging() {
    let test_cases = [
        SchedulerWakeTestCase {
            description: "SQLite real-clock wake",
            backend: Backend::Sqlite,
            expected_ticks: 1,
            expected_max_decision_delay: TimeDelta::seconds(3),
            launcher_poll: Duration::from_millis(700),
        },
        SchedulerWakeTestCase {
            description: "PostgreSQL real-clock wake",
            backend: Backend::Postgres,
            expected_ticks: 1,
            expected_max_decision_delay: TimeDelta::seconds(3),
            launcher_poll: Duration::from_millis(700),
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let location = create_location(store, "orders", "test-admin")
                .await
                .expect("location");
            let base = Utc::now()
                .duration_trunc(TimeDelta::seconds(1))
                .expect("second");
            let due = base + TimeDelta::seconds(3);
            let trigger_id = publish_calendar(store, location, (base, &[3])).await;
            let published = Utc::now();
            assert!(
                published < due,
                "{}: setup ran {:?} past the due time, so nothing could tick",
                test_case.description,
                published - due
            );
            let (ticks, cycles) = run_scheduler_until(
                store,
                trigger_id,
                (
                    due + test_case.expected_max_decision_delay + TimeDelta::seconds(1),
                    test_case.launcher_poll,
                ),
            )
            .await;
            println!(
                "{}: published {:?} after base, ticks {ticks:?}",
                test_case.description,
                published - base,
            );
            for cycle in &cycles {
                println!(
                    "{}: cycle {:?}, polled {}",
                    test_case.description, cycle.timings, cycle.polled_at
                );
            }
            assert_eq!(
                ticks.len(),
                test_case.expected_ticks,
                "{}",
                test_case.description
            );
            assert!(
                ticks[0].evaluated_at - due <= test_case.expected_max_decision_delay,
                "{}: decided {:?} after the due time",
                test_case.description,
                ticks[0].evaluated_at - due
            );
            close_test_store(fixture).await;
        });
    }
}
