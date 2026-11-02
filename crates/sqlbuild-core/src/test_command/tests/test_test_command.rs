use crate::cli_text::models::CliStyle;
use crate::test_command::main::parameterized_test_label::parameterized_test_label;
use crate::test_command::main::render_test_assumptions::render_test_assumptions;
use crate::test_command::main::render_test_completion::render_test_completion;
use crate::test_command::main::render_test_inspection::render_test_inspection;
use crate::test_command::main::render_test_ready::render_test_ready;
use crate::test_command::main::test_exit_code::test_exit_code;
use crate::test_command::main::test_group_name::test_group_name;
use crate::test_command::main::test_name_width::test_name_width;
use crate::test_command::main::test_outcome_status::test_outcome_status;
use crate::test_command::models::{TestFailure, TestInspectionEntry, TestPlanWarning};
use crate::test_command::tests::helpers::parameter;
use crate::test_command::tests::test_types::{
    AssumptionsTestCase, CompletionTestCase, GroupTestCase, InspectionTestCase, LabelTestCase,
    OutcomeTestCase, ReadyTestCase, WidthTestCase,
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
        OutcomeTestCase {
            description: "a local conversion refusal is REFUSED",
            outcome: "refused",
            expected_status: "REFUSED",
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
fn given_names_when_resolving_width_then_widens_for_indented_children() {
    let test_cases = [
        WidthTestCase {
            description: "short names use the minimum width",
            labels: vec!["orders_assert".to_owned()],
            child_names: Vec::new(),
            expected_width: 50,
        },
        WidthTestCase {
            description: "a long child name widens by its indent",
            labels: vec!["orders_assert".to_owned()],
            child_names: vec!["x".repeat(60)],
            expected_width: 64,
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
fn given_counts_when_rendering_ready_then_writes_surface_header() {
    let test_cases = [ReadyTestCase {
        description: "counts tests and models",
        counts: (1, 2),
        expected_text: "\nTest ready  1 selected, 2 models\n\n",
    }];

    for test_case in test_cases {
        assert_eq!(
            render_test_ready(test_case.counts, PLAIN),
            test_case.expected_text,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_results_when_completing_then_renders_summary_failures_and_exit_code() {
    let test_cases = [
        CompletionTestCase {
            description: "all passing writes only the summary",
            counts: (2, 2),
            failures: Vec::new(),
            expected_text: "\nPASS=2  FAIL=0  TOTAL=2\n",
            expected_exit_code: 0,
        },
        CompletionTestCase {
            description: "a coded failure renders its code, muted lines and help",
            counts: (0, 1),
            failures: vec![TestFailure {
                label: "orders_assert".to_owned(),
                error_message: Some("first\nsecond".to_owned()),
                error_code: Some("T003".to_owned()),
                error_help: Some("fix it".to_owned()),
            }],
            expected_text: "\nPASS=0  FAIL=1  TOTAL=1\n\nFailures:\n\n  orders_assert\n    error     [T003] first\n              second\n                = help: fix it\n",
            expected_exit_code: 1,
        },
        CompletionTestCase {
            description: "an uncoded empty failure says the test failed",
            counts: (1, 2),
            failures: vec![TestFailure {
                label: "orders_unique".to_owned(),
                error_message: Some(String::new()),
                ..TestFailure::default()
            }],
            expected_text: "\nPASS=1  FAIL=1  TOTAL=2\n\nFailures:\n\n  orders_unique\n    error     test failed\n",
            expected_exit_code: 1,
        },
        CompletionTestCase {
            description: "a refused test is counted apart from failures and never passes",
            counts: (1, 2),
            failures: vec![TestFailure {
                label: "orders_local".to_owned(),
                error_message: Some("refused".to_owned()),
                error_code: Some("T006".to_owned()),
                error_help: None,
            }],
            expected_text: "\nPASS=1  FAIL=0  REFUSED=1  TOTAL=2\n\nFailures:\n\n  orders_local\n    error     [T006] refused\n",
            expected_exit_code: 1,
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            render_test_completion(test_case.counts, &test_case.failures, PLAIN),
            test_case.expected_text,
            "{}",
            test_case.description
        );
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

#[test]
fn given_local_results_when_summarising_assumptions_then_counts_tests_per_assumption() {
    let test_cases = [
        AssumptionsTestCase {
            description: "no assumption writes nothing",
            assumptions: vec![Vec::new(), Vec::new()],
            strict: false,
            expected_text: "",
        },
        AssumptionsTestCase {
            description: "each assumption lists the tests relying on it, sorted by name",
            assumptions: vec![
                vec!["session_timezone_utc".to_owned()],
                vec![
                    "source_variant_json".to_owned(),
                    "session_timezone_utc".to_owned(),
                ],
            ],
            strict: false,
            expected_text: "\nAssumptions relied on (rerun with --strict to refuse them):\n  session_timezone_utc  2 tests\n  source_variant_json   1 test\n",
        },
        AssumptionsTestCase {
            description: "in strict mode the assumptions left are the ones the target confirmed",
            assumptions: vec![vec!["session_timezone_utc".to_owned()]],
            strict: true,
            expected_text: "\nAssumptions relied on (confirmed by the target, so strict mode keeps them):\n  session_timezone_utc  1 test\n",
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            render_test_assumptions(&test_case.assumptions, PLAIN, test_case.strict),
            test_case.expected_text,
            "{}",
            test_case.description
        );
    }
}
