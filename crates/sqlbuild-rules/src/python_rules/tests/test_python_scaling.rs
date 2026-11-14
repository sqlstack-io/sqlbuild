use std::time::Duration;

use serde_json::json;

use crate::python_rules::tests::helpers::{scaling_project, timed_python_codes};
use crate::python_rules::tests::test_types::PythonScalingTestCase;

#[test]
fn given_many_modules_and_dense_findings_when_evaluating_python_rules_then_finishes_within_budget()
{
    let test_cases = [
        PythonScalingTestCase {
            description: "default rules over 4000 modules and 40 hooks of 300 findings each",
            units: 4000,
            noisy_files: 40,
            functions: 150,
            config: json!({ "select": ["SQBRPYTHON"] }),
            expected_annotation_faults: 12000,
            expected_max_duration: Duration::from_secs(5),
        },
        PythonScalingTestCase {
            description: "the same project with unreachable definitions walking every import",
            units: 4000,
            noisy_files: 40,
            functions: 150,
            config: json!({ "select": ["SQBRPYTHON", "SQBRPYTHON306"] }),
            expected_annotation_faults: 12000,
            expected_max_duration: Duration::from_secs(5),
        },
    ];
    for test_case in test_cases {
        let files = scaling_project(test_case.units, test_case.noisy_files, test_case.functions);
        let (codes, elapsed) = timed_python_codes(&files, test_case.config).expect("rules run");
        let annotation_faults: usize = codes.iter().filter(|code| *code == "SQBRPYTHON101").count();
        assert_eq!(
            annotation_faults, test_case.expected_annotation_faults,
            "{}",
            test_case.description
        );
        assert!(
            elapsed < test_case.expected_max_duration,
            "{}: took {elapsed:?}",
            test_case.description
        );
        eprintln!(
            "{}: {elapsed:?}, {} faults",
            test_case.description,
            codes.len()
        );
    }
}
