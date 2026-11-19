use serde_json::{Value, json};
use sqlbuild_orchestration::platform::constants::PLAN_DOCUMENT_BYTES;

use crate::native_commands::_helpers::location::plan_document::plan_document;
use crate::native_commands::_helpers::location::work::next_work_check;
use crate::native_commands::constants::LOCATION_WORK_IDLE_POLL;
use crate::native_commands::models::LocationWorkTurn;
use crate::native_commands::tests::helpers::plan_output;
use crate::native_commands::tests::test_types::{PlanDocumentTestCase, WorkCheckTestCase};

#[test]
fn given_plan_output_when_reading_it_on_a_location_then_diffs_always_travel_beside_the_document() {
    let test_cases = [
        PlanDocumentTestCase {
            description: "a small plan's diffs leave the document with their counts too",
            output: plan_output("+SELECT 2", 2),
            planned: true,
            expected_first_model: json!({"name": "model_0", "reason": "query_changed", "diff_stats": {
                "query": {"added": 1, "removed": 0, "whitespace_only": false},
                "config": {"added": 1, "removed": 0, "whitespace_only": false},
            }}),
            expected_diffs: (2, Some(vec!["+SELECT 2".to_owned()])),
        },
        PlanDocumentTestCase {
            description: "a plan over the document cap keeps every diff beside it",
            output: plan_output(
                &format!("+{}", "x".repeat(1024)),
                PLAN_DOCUMENT_BYTES / 1024 + 1,
            ),
            planned: true,
            expected_first_model: json!({"name": "model_0", "reason": "query_changed", "diff_stats": {
                "query": {"added": 1, "removed": 0, "whitespace_only": false},
                "config": {"added": 1, "removed": 0, "whitespace_only": false},
            }}),
            expected_diffs: (
                PLAN_DOCUMENT_BYTES / 1024 + 1,
                Some(vec![format!("+{}", "x".repeat(1024))]),
            ),
        },
        PlanDocumentTestCase {
            description: "a failed sqb plan is not read as a plan",
            output: plan_output("+SELECT 2", 1),
            planned: false,
            expected_first_model: Value::Null,
            expected_diffs: (0, None),
        },
    ];
    for test_case in test_cases {
        let (document, diffs) = plan_document(&test_case.output, test_case.planned);
        assert_eq!(
            (
                document["models"][0].clone(),
                diffs.len(),
                diffs
                    .get("model_0")
                    .and_then(|diff| diff.query_diff.clone()),
            ),
            (
                test_case.expected_first_model,
                test_case.expected_diffs.0,
                test_case.expected_diffs.1
            ),
            "{description}",
            description = test_case.description
        );
    }
}

#[test]
fn given_a_turn_of_location_work_when_scheduling_the_next_check_then_only_an_idle_turn_waits() {
    let test_cases = [
        WorkCheckTestCase {
            description: "a finished plan checks at once, as a person often plans again",
            claimed: true,
            sync: false,
            expected_wait: std::time::Duration::ZERO,
        },
        WorkCheckTestCase {
            description: "a requested sync checks at once too",
            claimed: true,
            sync: true,
            expected_wait: std::time::Duration::ZERO,
        },
        WorkCheckTestCase {
            description: "nothing to claim waits the idle interval",
            claimed: false,
            sync: false,
            expected_wait: LOCATION_WORK_IDLE_POLL,
        },
    ];
    let now = tokio::time::Instant::now();
    for test_case in test_cases {
        let turn = LocationWorkTurn {
            files: Vec::new(),
            sync: test_case.sync,
            claimed: test_case.claimed,
        };
        assert_eq!(
            next_work_check(&turn, now) - now,
            test_case.expected_wait,
            "{}",
            test_case.description
        );
    }
}
