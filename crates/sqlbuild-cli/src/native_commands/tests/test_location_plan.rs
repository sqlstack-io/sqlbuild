use serde_json::{Value, json};
use sqlbuild_orchestration::platform::constants::PLAN_DOCUMENT_BYTES;

use crate::native_commands::main::_plan_document::plan_document;
use crate::native_commands::tests::helpers::plan_output;
use crate::native_commands::tests::test_types::PlanDocumentTestCase;

#[test]
fn given_plan_output_when_reading_it_on_a_location_then_only_oversized_diffs_are_dropped() {
    let test_cases = [
        PlanDocumentTestCase {
            description: "a plan under the cap keeps its diffs",
            output: plan_output("+SELECT 2", 2),
            planned: true,
            expected_first_model: json!({"name": "model_0", "reason": "query_changed", "query_diff": ["+SELECT 2"], "config_diff": ["+a"]}),
        },
        PlanDocumentTestCase {
            description: "diffs over the cap are dropped and the plan is kept",
            output: plan_output(&"x".repeat(1024), PLAN_DOCUMENT_BYTES / 1024 + 1),
            planned: true,
            expected_first_model: json!({"name": "model_0", "reason": "query_changed"}),
        },
        PlanDocumentTestCase {
            description: "a failed sqb plan is not read as a plan",
            output: plan_output("+SELECT 2", 1),
            planned: false,
            expected_first_model: Value::Null,
        },
    ];
    for test_case in test_cases {
        let document: Value = plan_document(&test_case.output, test_case.planned);
        assert_eq!(
            document["models"][0],
            test_case.expected_first_model,
            "{description}",
            description = test_case.description
        );
    }
}
