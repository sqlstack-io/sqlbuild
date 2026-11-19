use crate::build_progress::main::batch_progress_message::batch_progress_message;
use crate::build_progress::main::format_build_footer::format_build_footer;
use crate::build_progress::main::progress_name_width::progress_name_width;
use crate::build_progress::main::render_build_spinner_line::render_build_spinner_line;
use crate::build_progress::main::render_failed_test_result::render_failed_test_result;
use crate::build_progress::main::render_model_result::render_model_result;
use crate::build_progress::main::scheduler_diagnostic_message::scheduler_diagnostic_message;
use crate::build_progress::models::{
    BatchProgressText, BuildResultText, ErrorText, LifecycleEventText, ModelResultText,
    ModelRowFacts, NameWidthInputs, ProgressLayout, PythonNodeText, SchedulerStateText, SpinnerRow,
};
use crate::build_progress::tests::helpers::{audit, failed_model, layout, model, test};
use crate::build_progress::tests::test_types::{
    BatchProgressTestCase, FailedTestTestCase, FooterTestCase, InlineErrorTestCase,
    ModelRowTestCase, NameWidthTestCase, SchedulerTestCase, SpinnerTestCase,
};
use crate::cli_text::main::format_inline_error_lines::format_inline_error_lines;
use crate::cli_text::models::CliStyle;

const PLAIN: CliStyle = CliStyle { use_color: false };

#[test]
fn given_build_results_when_formatting_footer_then_matches_python_layout() {
    let test_cases = [
        FooterTestCase {
            description: "a clean run reports success",
            result: BuildResultText {
                models: vec![model("orders", "success")],
                ..BuildResultText::default()
            },
            python: Vec::new(),
            use_color: false,
            expected_text: "\u{2713} Completed successfully  PASS=1  WARN=0  FAIL=0  INSUFFICIENT=0  SKIP=0  TOTAL=1  (1.50s)",
        },
        FooterTestCase {
            description: "warnings, failures and skipped nodes each get a section",
            result: BuildResultText {
                failed: true,
                models: vec![
                    ModelResultText {
                        audits: vec![audit("not_null", "warn", 2)],
                        ..model("orders", "success")
                    },
                    ModelResultText {
                        staging_relation: Some("orders__delta".to_owned()),
                        ..failed_model("customers", "boom")
                    },
                    model("payments", "skipped"),
                ],
                ..BuildResultText::default()
            },
            python: vec![PythonNodeText {
                name: "refresh".to_owned(),
                kind: "asset".to_owned(),
                status: "failed".to_owned(),
                error_message: None,
                warnings: Vec::new(),
            }],
            use_color: false,
            expected_text: "\nWarnings:\n\n  orders\n    audit not_null returned 2 rows\n\n\u{2717} Completed with errors  PASS=1  WARN=1  FAIL=2  INSUFFICIENT=0  SKIP=1  TOTAL=5  (1.50s)\n\nFailures:\n\n  customers  (staging)\n    error     [E100] boom\n    delta table kept for inspection: orders__delta\n\n\nPython node failures:\n\n  asset     refresh                                            Python node failed\n\nSkipped (1): payments",
        },
    ];

    for test_case in test_cases {
        let text: String = format_build_footer(
            &test_case.result,
            &test_case.python,
            1.5,
            CliStyle {
                use_color: test_case.use_color,
            },
        );
        assert_eq!(text, test_case.expected_text, "{}", test_case.description);
    }
}

#[test]
fn given_model_result_when_rendering_then_nests_tests_audits_and_errors() {
    let test_cases = [
        ModelRowTestCase {
            description: "a passing model lists its tests and aggregated audits",
            model: ModelResultText {
                audits: vec![audit("unique", "error", 1), audit("unique", "pass", 0)],
                ..model("orders", "success")
            },
            tests: vec![test("orders_test", "pass", "pass")],
            verbose: false,
            expected_text: "  2/3  table     orders                   OK     1.23s\n         test      orders_test            PASS\n           expect  expected orders        PASS\n         audit     unique                 FAIL  1/2  1 row\n",
        },
        ModelRowTestCase {
            description: "a failed model shows its phase and wrapped coded error",
            model: failed_model("customers", "boom"),
            tests: Vec::new(),
            verbose: false,
            expected_text: "  2/3  table     customers                FAIL   1.23s  staging\n       error     [E100] boom\n",
        },
        ModelRowTestCase {
            description: "verbose output prints lifecycle SQL with a semicolon",
            model: ModelResultText {
                events: vec![LifecycleEventText::Sql("select 1  ".to_owned())],
                batch_count: Some(2),
                rows_affected: Some(12_345),
                ..model("orders", "success")
            },
            tests: Vec::new(),
            verbose: true,
            expected_text: "  2/3  table     orders                   OK     1.23s\n\n    select 1;\n\n         2 batches    12.3K rows\n",
        },
    ];

    for test_case in test_cases {
        let facts: ModelRowFacts = ModelRowFacts {
            display_type: "table".to_owned(),
            ..ModelRowFacts::default()
        };
        assert_eq!(
            render_model_result(
                (&test_case.model, &facts, &test_case.tests),
                2,
                layout(test_case.verbose),
                PLAIN
            ),
            test_case.expected_text,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_failed_test_when_rendering_then_draws_tree_children() {
    let test_cases = [
        FailedTestTestCase {
            description: "the last expectation closes the tree",
            test: test("orders_test", "fail", "fail"),
            expected_text: "       test      orders_test              FAIL\n           \u{2514}\u{2500}\u{2500} expect  expected orders    FAIL\n",
        },
        FailedTestTestCase {
            description: "an error closes the tree after the expectations",
            test: crate::build_progress::models::TestResultText {
                error: ErrorText {
                    code: None,
                    message: Some("broken".to_owned()),
                    help: None,
                },
                ..test("orders_test", "error", "error")
            },
            expected_text: "       test      orders_test              ERROR\n           \u{251c}\u{2500}\u{2500} expect  expected orders    ERROR\n           \u{2514}\u{2500}\u{2500} error   broken\n",
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            render_failed_test_result(&test_case.test, layout(false), PLAIN),
            test_case.expected_text,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_running_node_when_drawing_spinner_then_fits_terminal() {
    let test_cases = [
        SpinnerTestCase {
            description: "a wide terminal shows the full row",
            row: SpinnerRow {
                position: 1,
                resource_kind: "table".to_owned(),
                name: "orders".to_owned(),
                message: "batch 1/2".to_owned(),
            },
            columns: 120,
            expected_text: "\r\x1b[K  1/3  table     orders  batch 1/2        \u{280b}",
        },
        SpinnerTestCase {
            description: "a narrow terminal truncates the row",
            row: SpinnerRow {
                position: 1,
                resource_kind: "table".to_owned(),
                name: "orders".to_owned(),
                message: String::new(),
            },
            columns: 24,
            expected_text: "\r\x1b[K  1/3  table     o... \u{280b}",
        },
    ];

    for test_case in test_cases {
        let layout: ProgressLayout = ProgressLayout {
            columns: test_case.columns,
            ..layout(false)
        };
        assert_eq!(
            render_build_spinner_line(&test_case.row, 0, layout, PLAIN),
            test_case.expected_text,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_row_names_when_sizing_then_tree_children_keep_status_column() {
    let test_cases = [
        NameWidthTestCase {
            description: "short names use the minimum width",
            inputs: NameWidthInputs {
                names: vec!["orders".to_owned()],
                tree_child_names: Vec::new(),
            },
            expected_width: 20,
        },
        NameWidthTestCase {
            description: "expectation children add their tree indent",
            inputs: NameWidthInputs {
                names: vec!["orders".to_owned()],
                tree_child_names: vec!["a_rather_long_model_name".to_owned()],
            },
            expected_width: 39,
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            progress_name_width(&test_case.inputs),
            test_case.expected_width,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_scheduler_state_when_describing_then_flags_frontier_constraints() {
    let test_cases = [
        SchedulerTestCase {
            description: "an idle worker with only waiting nodes is frontier constrained",
            state: SchedulerStateText {
                running: 1,
                ready: 0,
                waiting: 3,
                limit: 4,
                aborted: 0,
            },
            expected_text: "Scheduler  1 running, 0 ready, 3 waiting on dependencies, limit 4  (DAG frontier constrained)",
        },
        SchedulerTestCase {
            description: "aborted nodes are reported after a stop",
            state: SchedulerStateText {
                running: 0,
                ready: 1,
                waiting: 2,
                limit: 4,
                aborted: 5,
            },
            expected_text: "Scheduler  0 running, 1 ready, 2 waiting on dependencies, 5 aborted after stop, limit 4",
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            scheduler_diagnostic_message(test_case.state),
            test_case.expected_text,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_long_error_when_wrapping_inline_then_truncates_to_two_lines() {
    let test_cases = [
        InlineErrorTestCase {
            description: "a coded error with help wraps and keeps the code first",
            error: (Some("E1"), "short", Some("do this")),
            width: 40,
            expected_lines: &["[E1] short", "= help: do this"],
        },
        InlineErrorTestCase {
            description: "hyphenated words are not split and overflow is truncated",
            error: (None, "alpha-beta-gamma delta epsilon zeta eta", None),
            width: 12,
            expected_lines: &["alpha-beta-g", "amma delt..."],
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            format_inline_error_lines(test_case.error, test_case.width, PLAIN),
            test_case.expected_lines,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_completed_batch_when_describing_then_names_model_window_rows_and_time() {
    let batch = |rows_affected: Option<i64>| BatchProgressText {
        model_name: "daily_orders".to_owned(),
        batch_number: 2,
        batch_count: 12,
        window: "2024-02-01..2024-03-01".to_owned(),
        rows_affected,
        elapsed_seconds: 1.26,
    };
    let test_cases = [
        BatchProgressTestCase {
            description: "known row count",
            batch: batch(Some(40)),
            expected_text: "Batch  daily_orders  2/12  2024-02-01..2024-03-01  40 rows  1.3s",
        },
        BatchProgressTestCase {
            description: "unknown row count",
            batch: batch(None),
            expected_text: "Batch  daily_orders  2/12  2024-02-01..2024-03-01  1.3s",
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            batch_progress_message(&test_case.batch),
            test_case.expected_text,
            "{}",
            test_case.description
        );
    }
}
