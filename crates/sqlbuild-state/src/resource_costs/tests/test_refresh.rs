use sqlbuild_core::cost_command::models::CostCoverage;

use crate::resource_costs::main::refresh_candidates::refresh_candidates;
use crate::resource_costs::main::resource_cost_observation_id::resource_cost_observation_id;
use crate::resource_costs::models::{ResourceCostKey, RunCostAttempt};
use crate::resource_costs::tests::test_types::{ObservationIdTestCase, RefreshTestCase};

const NOW: i64 = 1_800_000_000;
const HOUR: i64 = 3_600;
const DAY: i64 = 86_400;

fn run(run_id: &str, attempt: i64, status: &str, age: i64) -> RunCostAttempt {
    RunCostAttempt {
        run_id: run_id.to_owned(),
        orchestration_run_id: None,
        attempt,
        coverage: CostCoverage {
            status: status.to_owned(),
            expected_statement_count: 2,
            observed_statement_count: 1,
            query_count: 1,
            limitations: Vec::new(),
        },
        target_name: None,
        run_started_at: String::new(),
        run_completed_at: String::new(),
        started_epoch: NOW - age,
    }
}

#[test]
fn given_latest_run_rows_when_choosing_refreshes_then_only_bounded_recent_incomplete_runs() {
    let test_cases = [
        RefreshTestCase {
            description: "pending and partial runs are re-read; complete and unavailable ones never",
            latest: vec![
                run("pending", 1, "pending", HOUR),
                run("partial", 4, "partial", 2 * HOUR),
                run("complete", 1, "complete", HOUR),
                run("unavailable", 1, "unavailable", HOUR),
            ],
            expected_runs: vec!["pending", "partial"],
        },
        RefreshTestCase {
            description: "a failed collection is retried until its third attempt, not after",
            latest: vec![
                run("second_failure", 2, "collection_failed", HOUR),
                run("third_failure", 3, "collection_failed", HOUR),
            ],
            expected_runs: vec!["second_failure"],
        },
        RefreshTestCase {
            description: "runs past Snowflake's query history window are left alone",
            latest: vec![
                run("inside", 1, "partial", 6 * DAY - HOUR),
                run("outside", 1, "partial", 6 * DAY + HOUR),
            ],
            expected_runs: vec!["inside"],
        },
        RefreshTestCase {
            description: "the newest three are re-read first",
            latest: (1..=5)
                .map(|age| run(&format!("run_{age}"), 1, "pending", age * HOUR))
                .collect(),
            expected_runs: vec!["run_1", "run_2", "run_3"],
        },
    ];
    for test_case in test_cases {
        let runs: Vec<String> = refresh_candidates(&test_case.latest, NOW)
            .into_iter()
            .map(|run| run.run_id)
            .collect();
        assert_eq!(runs, test_case.expected_runs, "{}", test_case.description);
    }
}

fn observation_id(attempt: i64, cluster_number: Option<i64>) -> String {
    resource_cost_observation_id(&ResourceCostKey {
        run_id: "run",
        attempt,
        resource_type: "model",
        resource_name: "orders",
        warehouse_name: "BUILD_WH",
        warehouse_size: "X-Small",
        warehouse_type: Some("STANDARD"),
        cluster_number,
    })
    .expect("identity encodes")
}

#[test]
fn given_cost_keys_when_identifying_observations_then_each_attempt_and_cluster_is_its_own_row() {
    let base: String = observation_id(1, None);
    let test_cases = [
        ObservationIdTestCase {
            description: "a retried append of the same attempt writes the same row",
            attempt: 1,
            cluster_number: None,
            expected_same_as_base: true,
        },
        ObservationIdTestCase {
            description: "a refresh is a new observation",
            attempt: 2,
            cluster_number: None,
            expected_same_as_base: false,
        },
        ObservationIdTestCase {
            description: "cluster 0 differs from an unknown cluster",
            attempt: 1,
            cluster_number: Some(0),
            expected_same_as_base: false,
        },
    ];
    for test_case in test_cases {
        let same: bool = observation_id(test_case.attempt, test_case.cluster_number) == base;
        assert_eq!(
            same, test_case.expected_same_as_base,
            "{}",
            test_case.description
        );
    }
}
