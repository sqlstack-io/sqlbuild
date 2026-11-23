use crate::contract_command::main::contract_exit_code::contract_exit_code;
use crate::contract_command::main::contract_progress_message::contract_progress_message;
use crate::contract_command::main::render_contract_json::render_contract_json;
use crate::contract_command::main::render_contract_text::render_contract_text;
use crate::contract_command::models::{
    ContractFindingText, ContractProgress, ContractReport, ContractResourceText,
};
use crate::contract_command::tests::test_types::{MessageTestCase, ReportTestCase};

#[test]
fn given_comparison_when_rendering_then_reports_text_json_and_exit_code() {
    let test_cases = [
        ReportTestCase {
            description: "a matching source after a write",
            report: ContractReport {
                from_target: "prod".to_owned(),
                resources: vec![ContractResourceText {
                    resource_type: "source".to_owned(),
                    resource_name: "raw_orders".to_owned(),
                    relation: "raw.orders".to_owned(),
                    findings: Vec::new(),
                }],
                written_paths: vec!["sql/sources/raw.yml".to_owned()],
            },
            expected_text: "Contract comparison from target 'prod'\n  OK    source:raw_orders\n\nUpdated repository declarations:\n  sql/sources/raw.yml\n\n0 contract difference(s)\n",
            expected_json: "{\n  \"differences\": 0,\n  \"from_target\": \"prod\",\n  \"resources\": [\n    {\n      \"findings\": [],\n      \"relation\": \"raw.orders\",\n      \"resource_name\": \"raw_orders\",\n      \"resource_type\": \"source\"\n    }\n  ],\n  \"written_paths\": [\n    \"sql/sources/raw.yml\"\n  ]\n}\n",
            expected_exit_code: 0,
        },
        ReportTestCase {
            description: "a model with one type difference",
            report: ContractReport {
                from_target: "prod".to_owned(),
                resources: vec![ContractResourceText {
                    resource_type: "model".to_owned(),
                    resource_name: "orders".to_owned(),
                    relation: "prod.orders".to_owned(),
                    findings: vec![ContractFindingText {
                        kind: "type_mismatch".to_owned(),
                        column: Some("id".to_owned()),
                        declared_type: Some("VARCHAR".to_owned()),
                        physical_type: Some("INTEGER".to_owned()),
                        message: "id differs".to_owned(),
                    }],
                }],
                written_paths: Vec::new(),
            },
            expected_text: "Contract comparison from target 'prod'\n  DIFF  model:orders\n        type_mismatch: id differs\n\n1 contract difference(s)\n",
            expected_json: "{\n  \"differences\": 1,\n  \"from_target\": \"prod\",\n  \"resources\": [\n    {\n      \"findings\": [\n        {\n          \"column\": \"id\",\n          \"declared_type\": \"VARCHAR\",\n          \"kind\": \"type_mismatch\",\n          \"message\": \"id differs\",\n          \"physical_type\": \"INTEGER\"\n        }\n      ],\n      \"relation\": \"prod.orders\",\n      \"resource_name\": \"orders\",\n      \"resource_type\": \"model\"\n    }\n  ],\n  \"written_paths\": []\n}\n",
            expected_exit_code: 1,
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            render_contract_text(&test_case.report),
            test_case.expected_text,
            "{}",
            test_case.description
        );
        assert_eq!(
            render_contract_json(&test_case.report).ok().as_deref(),
            Some(test_case.expected_json),
            "{}",
            test_case.description
        );
        assert_eq!(
            contract_exit_code(test_case.report.difference_count()),
            test_case.expected_exit_code,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_counts_when_reporting_progress_then_pluralizes_and_times() {
    let test_cases = [
        MessageTestCase {
            description: "one contract starts",
            event: ContractProgress::InspectionStart {
                count: 1,
                target: "prod".to_owned(),
            },
            expected_message: "Inspecting 1 contract from target 'prod'...",
        },
        MessageTestCase {
            description: "two contracts complete",
            event: ContractProgress::InspectionComplete {
                count: 2,
                target: "prod".to_owned(),
                seconds: 0.125,
            },
            expected_message: "Inspected 2 contracts from target 'prod' (0.12s)",
        },
        MessageTestCase {
            description: "inspection fails",
            event: ContractProgress::InspectionFailed { seconds: 1.5 },
            expected_message: "Contract inspection failed after 1.50s",
        },
        MessageTestCase {
            description: "one file written",
            event: ContractProgress::WriteComplete {
                count: 1,
                seconds: 0.0,
            },
            expected_message: "Updated repository contracts (1 file, 0.00s)",
        },
        MessageTestCase {
            description: "write fails",
            event: ContractProgress::WriteFailed { seconds: 2.0 },
            expected_message: "Repository contract update failed after 2.00s",
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            contract_progress_message(&test_case.event),
            test_case.expected_message,
            "{}",
            test_case.description
        );
    }
}
