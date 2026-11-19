use std::collections::BTreeMap;

use serde_json::{Value as Json, json};

use crate::argv::main::job_run_arguments::job_run_arguments;
use crate::argv::tests::test_types::JobRunArgumentsTestCase;

#[test]
fn given_a_server_job_run_when_building_its_arguments_then_it_names_the_job_and_leaves_its_own_settings()
 {
    let test_cases = [
        JobRunArgumentsTestCase {
            description: "concurrency and node-level settings are left to the build",
            settings: json!({"concurrency": 8, "run_tests": false, "full_refresh": true}),
            expected_arguments: &["--select", "job:orders_job"],
        },
        JobRunArgumentsTestCase {
            description: "the job's warehouse is the whole run's, setup and planning included",
            settings: json!({"concurrency": 8, "run_tests": false, "warehouse": "ORDERS_WH"}),
            expected_arguments: &["--warehouse=ORDERS_WH", "--select", "job:orders_job"],
        },
        JobRunArgumentsTestCase {
            description: "build-wide settings still travel as flags",
            settings: json!({"concurrency": 4, "vars": {"region": "eu"}, "fail_fast": true}),
            expected_arguments: &[
                "--fail-fast",
                "--vars={\"region\":\"eu\"}",
                "--select",
                "job:orders_job",
            ],
        },
    ];
    for test_case in test_cases {
        let settings: BTreeMap<String, Json> = test_case
            .settings
            .as_object()
            .map(|object| object.clone().into_iter().collect())
            .unwrap_or_default();
        let arguments: Vec<String> =
            job_run_arguments("orders_job", &settings).expect(test_case.description);
        let arguments: Vec<&str> = arguments.iter().map(String::as_str).collect();
        assert_eq!(
            arguments, test_case.expected_arguments,
            "{}",
            test_case.description
        );
    }
}
