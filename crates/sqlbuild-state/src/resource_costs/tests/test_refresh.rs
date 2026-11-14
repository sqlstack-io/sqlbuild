use crate::resource_costs::main::refresh_candidates::refresh_candidates;
use crate::resource_costs::tests::helpers::{NOW, observation_id, run};
use crate::resource_costs::tests::test_types::{ObservationIdTestCase, RefreshTestCase};

const HOUR: i64 = 3_600;
const DAY: i64 = 86_400;

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
