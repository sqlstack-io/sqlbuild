use crate::cli_text::models::CliStyle;
use crate::progress::models::ExpectationStep;
use crate::run_output::main::render_failures::render_failures;
use crate::test_command::main::expectation_summary::expectation_summary;
use crate::test_command::main::parameterized_test_label::parameterized_test_label;
use crate::test_command::main::render_test_inspection::render_test_inspection;
use crate::test_command::main::test_exit_code::test_exit_code;
use crate::test_command::main::test_failure_entry::test_failure_entry;
use crate::test_command::main::test_group_name::test_group_name;
use crate::test_command::main::test_name_width::test_name_width;
use crate::test_command::main::test_outcome_status::test_outcome_status;
use crate::test_command::models::{TestFailureFacts, TestInspectionEntry, TestPlanWarning};
use crate::test_command::tests::helpers::{facts, parameter, step};
use crate::test_command::tests::test_types::{
    EntryTestCase, ExitTestCase, GroupTestCase, InspectionTestCase, LabelTestCase, OutcomeTestCase,
    SummaryTestCase, WidthTestCase,
};

const PLAIN: CliStyle = CliStyle { use_color: false };

#[test]
fn given_test_when_labelling_then_adds_source_and_values_only_for_parameterized_cases() {
    let test_cases = [
        LabelTestCase {
            description: "a plain test keeps its name",
            source_path: Some("tests/unit/test_orders.sql"),
            parameters: Vec::new(),
            expected_label: "order_status",
        },
        LabelTestCase {
            description: "no source path keeps the name",
            source_path: None,
            parameters: vec![parameter("status", false, "\"open\"")],
            expected_label: "order_status",
        },
        LabelTestCase {
            description: "parameters render typed values and nullability",
            source_path: Some("tests/unit/test_orders.sql"),
            parameters: vec![
                parameter("status", false, "\"open\""),
                parameter("note", true, "null"),
            ],
            expected_label: "order_status (tests/unit/test_orders.sql; status:string=\"open\", note:string?=null)",
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            parameterized_test_label("order_status", test_case.source_path, &test_case.parameters),
            test_case.expected_label,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_chain_when_grouping_then_prefers_first_expected_model() {
    let test_cases = [
        GroupTestCase {
            description: "the first model with expected rows names the group",
            chain: vec![("stg".to_owned(), false), ("orders".to_owned(), true)],
            expected_group: "orders",
        },
        GroupTestCase {
            description: "without expected rows the first model names the group",
            chain: vec![("stg".to_owned(), false), ("orders".to_owned(), false)],
            expected_group: "stg",
        },
        GroupTestCase {
            description: "an empty chain is unknown",
            chain: Vec::new(),
            expected_group: "(unknown)",
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            test_group_name(&test_case.chain),
            test_case.expected_group,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_outcomes_when_rendering_status_then_distinguishes_errors_from_failures() {
    let test_cases = [
        OutcomeTestCase {
            description: "a pass is PASS",
            outcome: "pass",
            expected_status: "PASS",
        },
        OutcomeTestCase {
            description: "an execution error is ERROR",
            outcome: "error",
            expected_status: "ERROR",
        },
        OutcomeTestCase {
            description: "an assertion failure is FAIL",
            outcome: "fail",
            expected_status: "FAIL",
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            test_outcome_status(test_case.outcome),
            test_case.expected_status,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_labels_when_resolving_width_then_measures_names_without_group_prefix() {
    let test_cases = [
        WidthTestCase {
            description: "no labels have no width",
            labels: Vec::new(),
            child_names: Vec::new(),
            expected_width: 0,
        },
        WidthTestCase {
            description: "a row under its group drops the group prefix",
            labels: vec![
                (
                    "fct_orders__totals_match".to_owned(),
                    "fct_orders".to_owned(),
                ),
                ("orders_assert".to_owned(), String::new()),
            ],
            child_names: Vec::new(),
            expected_width: 13,
        },
        WidthTestCase {
            description: "a label that is only the prefix keeps its full name",
            labels: vec![("fct_orders__".to_owned(), "fct_orders".to_owned())],
            child_names: Vec::new(),
            expected_width: 12,
        },
        WidthTestCase {
            description: "a long child name widens the column so it fits under its row",
            labels: vec![("orders_assert".to_owned(), String::new())],
            child_names: vec!["no_negative_orders".to_owned()],
            expected_width: 24,
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            test_name_width(&test_case.labels, &test_case.child_names),
            test_case.expected_width,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_expectation_when_summarising_then_counts_differing_or_failing_rows() {
    let test_cases = [
        SummaryTestCase {
            description: "a passing expectation says nothing",
            step: step("fct_orders", "pass"),
            expected_summary: "",
        },
        SummaryTestCase {
            description: "missing and unexpected rows add up",
            step: ExpectationStep {
                missing_row_count: Some(1),
                unexpected_row_count: Some(2),
                ..step("fct_orders", "fail")
            },
            expected_summary: "3 rows differ",
        },
        SummaryTestCase {
            description: "without split counts the mismatched rows are counted",
            step: ExpectationStep {
                mismatched_row_count: 1,
                ..step("fct_orders", "fail")
            },
            expected_summary: "1 row differs",
        },
        SummaryTestCase {
            description: "an assertion counts the rows it returned",
            step: ExpectationStep {
                actual_row_count: 4,
                ..step("assertion no_negative_totals", "fail")
            },
            expected_summary: "4 rows",
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            expectation_summary(&test_case.step),
            test_case.expected_summary,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_failed_test_when_building_entry_then_renders_detail_and_short_reason() {
    let test_cases = [
        EntryTestCase {
            description: "an engine error with its statement and files",
            facts: TestFailureFacts {
                error_message: Some(
                    "Binder Error: Referenced column \"amount\" not found".to_owned(),
                ),
                statement: Some(("model".to_owned(), "CREATE".to_owned())),
                compiled_path: Some("target/compiled/fct_orders.sql".to_owned()),
                source_path: Some("tests/sql/fct_orders/totals.sql".to_owned()),
                ..facts("fct_orders__totals_match")
            },
            expected_text: "\nFailures:\n\n  fct_orders__totals_match  (test, local)\n    error     Binder Error: Referenced column \"amount\" not found\n    statement model · CREATE\n              target/compiled/fct_orders.sql\n    test      tests/sql/fct_orders/totals.sql\n",
            expected_reason: "Binder Error",
        },
        EntryTestCase {
            description: "a failing expectation shows its differing rows",
            facts: TestFailureFacts {
                steps: vec![ExpectationStep {
                    actual_row_count: 3,
                    expected_row_count: 3,
                    missing_row_count: Some(1),
                    unexpected_row_count: Some(1),
                    missing_samples: vec![vec![
                        ("order_id".to_owned(), "1001".to_owned()),
                        ("status".to_owned(), "shipped".to_owned()),
                    ]],
                    unexpected_samples: vec![vec![
                        ("order_id".to_owned(), "1001".to_owned()),
                        ("status".to_owned(), "refunded".to_owned()),
                    ]],
                    ..step("fct_orders", "fail")
                }],
                ..facts("fct_orders__totals_match")
            },
            expected_text: "\nFailures:\n\n  fct_orders__totals_match  (test, local)\n    error     expected rows of fct_orders differ: 1 missing, 1 extra (3\n              expected, 3 actual)\n                 order_id  status\n              -  1001      shipped  missing\n              +  1001      refunded  extra\n",
            expected_reason: "2 rows differ",
        },
        EntryTestCase {
            description: "a timeout says it timed out",
            facts: TestFailureFacts {
                error_message: Some("statement cancelled after 30s".to_owned()),
                timed_out: true,
                ..facts("fct_orders__totals_match")
            },
            expected_text: "\nFailures:\n\n  fct_orders__totals_match  (test, local)\n    error     statement cancelled after 30s\n",
            expected_reason: "timed out",
        },
        EntryTestCase {
            description: "no message and no failing expectation says the test failed",
            facts: facts("orders_unique"),
            expected_text: "\nFailures:\n\n  orders_unique  (test, local)\n    error     test failed\n",
            expected_reason: "test failed",
        },
    ];

    for test_case in test_cases {
        let (entry, reason) = test_failure_entry(&test_case.facts);
        let text = render_failures(&[entry], 80, PLAIN);
        assert_eq!(text, test_case.expected_text, "{}", test_case.description);
        assert_eq!(
            reason, test_case.expected_reason,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_counts_when_resolving_exit_code_then_fails_unless_every_test_passed() {
    let test_cases = [
        ExitTestCase {
            description: "every test passing exits zero",
            counts: (2, 2),
            expected_exit_code: 0,
        },
        ExitTestCase {
            description: "any test not passing exits one",
            counts: (1, 2),
            expected_exit_code: 1,
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            test_exit_code(test_case.counts.0, test_case.counts.1),
            test_case.expected_exit_code,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_plan_when_inspecting_then_lists_boundaries_and_counts_errors() {
    let entry = TestInspectionEntry {
        label: "order_status [open_case]".to_owned(),
        mocked_sources: vec!["raw_orders".to_owned()],
        mocked_refs: vec!["stg".to_owned()],
        real_models: vec!["orders".to_owned()],
        expected_models: vec!["orders".to_owned()],
        ..TestInspectionEntry::default()
    };
    let test_cases = [
        InspectionTestCase {
            description: "nothing selected completes",
            entries: Vec::new(),
            warnings: Vec::new(),
            expected_text: "\nResolved test plan\n\nTest plan inspection complete: 0 selected, 0 errors.\n",
            expected_exit_code: 0,
        },
        InspectionTestCase {
            description: "an error warning fails the inspection",
            entries: vec![entry],
            warnings: vec![
                TestPlanWarning {
                    severity: "warning".to_owned(),
                    message: "careful".to_owned(),
                },
                TestPlanWarning {
                    severity: "error".to_owned(),
                    message: "broken".to_owned(),
                },
            ],
            expected_text: "\nResolved test plan\n\norder_status [open_case]\n  mocked sources: raw_orders\n  mocked refs: stg\n  mocked seeds: none\n  mocked dbt refs: none\n  mocked table functions: none\n  real models: orders\n  expected models: orders\n  boundary: stg is replaced by __ref__stg\n\n  warning: careful\n  error: broken\nTest plan inspection failed: 1 selected, 1 errors.\n",
            expected_exit_code: 1,
        },
        InspectionTestCase {
            description: "a test selected by --since lists its reasons",
            entries: vec![TestInspectionEntry {
                label: "orders_rollup".to_owned(),
                real_models: vec!["stg_orders".to_owned()],
                selection_reasons: vec!["executes stg_orders (model SQL changed)".to_owned()],
                ..TestInspectionEntry::default()
            }],
            warnings: Vec::new(),
            expected_text: "\nResolved test plan\n\norders_rollup\n  mocked sources: none\n  mocked refs: none\n  mocked seeds: none\n  mocked dbt refs: none\n  mocked table functions: none\n  real models: stg_orders\n  expected models: none\n  selected: executes stg_orders (model SQL changed)\n\nTest plan inspection complete: 1 selected, 0 errors.\n",
            expected_exit_code: 0,
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            render_test_inspection(&test_case.entries, &test_case.warnings),
            (
                test_case.expected_text.to_owned(),
                test_case.expected_exit_code
            ),
            "{}",
            test_case.description
        );
    }
}
