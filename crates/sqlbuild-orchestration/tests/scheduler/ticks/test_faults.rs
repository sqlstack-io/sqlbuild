//! Two schedulers under random crashes, faults, failovers and frozen leaders tick every due time once.

use crate::scheduler::tests::contracts::ticks::helpers::fault_run;
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
        },
        SchedulerFaultTestCase {
            description: "PostgreSQL two-scheduler fault injection",
            backend: Backend::Postgres,
            expected_anomalies: FaultAnomalies::default(),
        },
    ];
    let seeds: u64 = std::env::var("SQB_SCHEDULER_FAULT_SEEDS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(3);
    for test_case in test_cases {
        for seed in 0..seeds {
            let Some((anomalies, counts)) = run(fault_run(test_case.backend, seed, 60)) else {
                return;
            };
            println!("{} seed {seed}: {counts:?}", test_case.description);
            assert_eq!(
                anomalies, test_case.expected_anomalies,
                "{} seed {seed}",
                test_case.description
            );
        }
    }
}
