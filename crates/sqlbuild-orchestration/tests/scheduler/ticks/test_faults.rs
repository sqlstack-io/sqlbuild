//! Two schedulers under random crashes, faults, failovers and frozen leaders tick every due time once.

use std::collections::BTreeMap;

use crate::scheduler::tests::contracts::ticks::helpers::{fault_run, tick_faults};
use crate::scheduler::tests::contracts::ticks::test_types::{
    FaultAnomalies, SchedulerFaultTestCase,
};
use crate::store::tests::store_helpers::run;
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_two_schedulers_under_random_faults_failovers_and_frozen_leaders_when_recovered_then_every_due_tick_happens_once()
 {
    let test_cases = [
        SchedulerFaultTestCase {
            description: "SQLite two-scheduler fault injection",
            backend: Backend::Sqlite,
            expected_anomalies: FaultAnomalies::default(),
            expected_points_without_a_crash: 0,
        },
        SchedulerFaultTestCase {
            description: "PostgreSQL two-scheduler fault injection",
            backend: Backend::Postgres,
            expected_anomalies: FaultAnomalies::default(),
            expected_points_without_a_crash: 0,
        },
    ];
    let seeds: u64 = std::env::var("SQB_SCHEDULER_FAULT_SEEDS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(5);
    for test_case in test_cases {
        let mut crashes: BTreeMap<&'static str, usize> = BTreeMap::new();
        let mut point = 0;
        for seed in 0..seeds {
            let Some((anomalies, counts, next)) =
                run(fault_run(test_case.backend, seed, (60, point)))
            else {
                return;
            };
            point = next;
            println!("{} seed {seed}: {counts:?}", test_case.description);
            assert_eq!(
                anomalies, test_case.expected_anomalies,
                "{} seed {seed}",
                test_case.description
            );
            for (point, count) in counts {
                *crashes.entry(point).or_default() += count;
            }
        }
        // Every crash point inside the tick transaction must actually fire across the seeds run.
        let unexercised: Vec<&str> = tick_faults()
            .iter()
            .map(|fault| fault.description)
            .filter(|point| crashes.get(point).copied().unwrap_or_default() == 0)
            .collect();
        println!("{} crashes per point: {crashes:?}", test_case.description);
        assert_eq!(
            unexercised.len(),
            test_case.expected_points_without_a_crash,
            "{}: {unexercised:?}",
            test_case.description
        );
    }
}
