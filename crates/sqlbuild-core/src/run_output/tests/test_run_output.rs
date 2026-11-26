//! Run output rendering at 80 and 160 columns and without a terminal.

use crate::cli_text::models::CliStyle;
use crate::run_output::main::cut_middle::cut_middle;
use crate::run_output::main::render_failed_list::render_failed_list;
use crate::run_output::main::render_failures::render_failures;
use crate::run_output::main::render_progress_footer::render_progress_footer;
use crate::run_output::main::render_progress_heartbeat::render_progress_heartbeat;
use crate::run_output::main::render_run_completion::render_run_completion;
use crate::run_output::main::render_run_header::render_run_header;
use crate::run_output::main::short_reason::short_reason;
use crate::run_output::models::RunCounts;
use crate::run_output::tests::helpers::{LONG_NAME, failed_list, failure_entry, footer, header};
use crate::run_output::tests::test_types::{
    CompletionTestCase, CutTestCase, FailedListTestCase, FailuresTestCase, FooterTestCase,
    HeaderTestCase, HeartbeatTestCase, ReasonTestCase,
};

const PLAIN: CliStyle = CliStyle { use_color: false };
const FRAME: &str = "\u{280b}";

#[test]
fn given_running_items_when_drawing_footer_then_fits_each_line_to_the_terminal() {
    let test_cases = [
        FooterTestCase {
            description: "nothing running shows the counts alone",
            footer: footer(0, 0, None),
            columns: 80,
            expected_lines: vec!["⠋ Testing  130/461   PASS=120  FAIL=3  NOT_RUN=7  (42.5s)"],
        },
        FooterTestCase {
            description: "80 columns cut the oldest name in the middle",
            footer: footer(4, 2, Some((LONG_NAME, 12.34))),
            columns: 80,
            expected_lines: vec![
                "⠋ Testing  130/461   PASS=120  FAIL=3  NOT_RUN=7  (42.5s)",
                "  4 running, 2 finished waiting to print · oldest 12.3s",
                "  └── fct_orders__totals_match_line_items...s_and_partial_shipments_are_applied",
            ],
        },
        FooterTestCase {
            description: "160 columns show the oldest name in full",
            footer: footer(4, 2, Some((LONG_NAME, 12.34))),
            columns: 160,
            expected_lines: vec![
                "⠋ Testing  130/461   PASS=120  FAIL=3  NOT_RUN=7  (42.5s)",
                "  4 running, 2 finished waiting to print · oldest 12.3s",
                "  └── fct_orders__totals_match_line_items_after_refunds_and_partial_shipments_are_applied",
            ],
        },
        FooterTestCase {
            description: "nothing held back leaves out the waiting count",
            footer: footer(1, 0, Some(("stg_orders__dedupes", 0.5))),
            columns: 80,
            expected_lines: vec![
                "⠋ Testing  130/461   PASS=120  FAIL=3  NOT_RUN=7  (42.5s)",
                "  1 running · oldest 0.5s",
                "  └── stg_orders__dedupes",
            ],
        },
    ];

    for test_case in test_cases {
        let lines = render_progress_footer(&test_case.footer, FRAME, test_case.columns, PLAIN);
        assert_eq!(lines, test_case.expected_lines, "{}", test_case.description);
    }
}

#[test]
fn given_no_terminal_when_writing_heartbeat_then_keeps_every_name_whole() {
    let test_cases = [
        HeartbeatTestCase {
            description: "the oldest running item follows the counts in full",
            footer: footer(4, 2, Some((LONG_NAME, 12.34))),
            expected_lines: vec![
                "  ... 130/461 tests  PASS=120  FAIL=3  NOT_RUN=7  4 running, 2 finished waiting to print  (42.5s)",
                "      oldest 12.3s  fct_orders__totals_match_line_items_after_refunds_and_partial_shipments_are_applied",
            ],
        },
        HeartbeatTestCase {
            description: "nothing running gives one line",
            footer: footer(0, 0, None),
            expected_lines: vec![
                "  ... 130/461 tests  PASS=120  FAIL=3  NOT_RUN=7  0 running  (42.5s)",
            ],
        },
    ];

    for test_case in test_cases {
        let lines = render_progress_heartbeat(&test_case.footer, PLAIN);
        assert_eq!(lines, test_case.expected_lines, "{}", test_case.description);
    }
}

#[test]
fn given_failed_items_when_rendering_failures_then_wraps_fields_to_the_terminal() {
    let test_cases = [
        FailuresTestCase {
            description: "no failures render nothing",
            entries: Vec::new(),
            columns: 80,
            expected_text: "",
        },
        FailuresTestCase {
            description: "80 columns wrap the message and keep the path whole",
            entries: vec![failure_entry("fct_orders__totals_match")],
            columns: 80,
            expected_text: "\nFailures:\n\n  fct_orders__totals_match  (test, local, 0.42s)\n    error     [T2001] expected rows differ from the model's rows: 1 row only\n              expected, 1 row only actual, compared on order_id and status\n    help      run with --verbose to see every differing row\n    model     fct_orders\n              tests/sql/fct_orders/totals.sql\n              order_id  status\n              - 1001      shipped  expected\n              + 1001      refunded  actual\n              12 | select amount / 0 as ratio\n                 |          ^^^^^^\n",
        },
        FailuresTestCase {
            description: "160 columns keep the message on one line",
            entries: vec![failure_entry("fct_orders__totals_match")],
            columns: 160,
            expected_text: "\nFailures:\n\n  fct_orders__totals_match  (test, local, 0.42s)\n    error     [T2001] expected rows differ from the model's rows: 1 row only expected, 1 row only actual, compared on order_id and status\n    help      run with --verbose to see every differing row\n    model     fct_orders\n              tests/sql/fct_orders/totals.sql\n              order_id  status\n              - 1001      shipped  expected\n              + 1001      refunded  actual\n              12 | select amount / 0 as ratio\n                 |          ^^^^^^\n",
        },
        FailuresTestCase {
            description: "a name too long for the heading puts the kind on its own line",
            entries: vec![failure_entry(LONG_NAME)],
            columns: 80,
            expected_text: "\nFailures:\n\n  fct_orders__totals_match_line_items_after_refunds_and_partial_shipments_are_applied\n    (test, local, 0.42s)\n    error     [T2001] expected rows differ from the model's rows: 1 row only\n              expected, 1 row only actual, compared on order_id and status\n    help      run with --verbose to see every differing row\n    model     fct_orders\n              tests/sql/fct_orders/totals.sql\n              order_id  status\n              - 1001      shipped  expected\n              + 1001      refunded  actual\n              12 | select amount / 0 as ratio\n                 |          ^^^^^^\n",
        },
    ];

    for test_case in test_cases {
        let text = render_failures(&test_case.entries, test_case.columns, PLAIN);
        assert_eq!(text, test_case.expected_text, "{}", test_case.description);
    }
}

#[test]
fn given_failed_items_when_listing_then_caps_at_twenty_unless_verbose() {
    let test_cases = [
        FailedListTestCase {
            description: "no failures render nothing",
            failed: Vec::new(),
            verbose: false,
            expected_text: "",
        },
        FailedListTestCase {
            description: "a few failures are all listed",
            failed: failed_list(2),
            verbose: false,
            expected_text: "\nFailed (2):\n  ✗ test_01  Binder Error\n  ✗ test_02  Binder Error\n",
        },
        FailedListTestCase {
            description: "more than twenty end with how many are hidden",
            failed: failed_list(22),
            verbose: false,
            expected_text: "\nFailed (22):\n  ✗ test_01  Binder Error\n  ✗ test_02  Binder Error\n  ✗ test_03  Binder Error\n  ✗ test_04  Binder Error\n  ✗ test_05  Binder Error\n  ✗ test_06  Binder Error\n  ✗ test_07  Binder Error\n  ✗ test_08  Binder Error\n  ✗ test_09  Binder Error\n  ✗ test_10  Binder Error\n  ✗ test_11  Binder Error\n  ✗ test_12  Binder Error\n  ✗ test_13  Binder Error\n  ✗ test_14  Binder Error\n  ✗ test_15  Binder Error\n  ✗ test_16  Binder Error\n  ✗ test_17  Binder Error\n  ✗ test_18  Binder Error\n  ✗ test_19  Binder Error\n  ✗ test_20  Binder Error\n\u{1b}[2m  ... and 2 more (use --verbose to show all)\u{1b}[0m\n",
        },
        FailedListTestCase {
            description: "--verbose lists every failure",
            failed: failed_list(22),
            verbose: true,
            expected_text: "\nFailed (22):\n  ✗ test_01  Binder Error\n  ✗ test_02  Binder Error\n  ✗ test_03  Binder Error\n  ✗ test_04  Binder Error\n  ✗ test_05  Binder Error\n  ✗ test_06  Binder Error\n  ✗ test_07  Binder Error\n  ✗ test_08  Binder Error\n  ✗ test_09  Binder Error\n  ✗ test_10  Binder Error\n  ✗ test_11  Binder Error\n  ✗ test_12  Binder Error\n  ✗ test_13  Binder Error\n  ✗ test_14  Binder Error\n  ✗ test_15  Binder Error\n  ✗ test_16  Binder Error\n  ✗ test_17  Binder Error\n  ✗ test_18  Binder Error\n  ✗ test_19  Binder Error\n  ✗ test_20  Binder Error\n  ✗ test_21  Binder Error\n  ✗ test_22  Binder Error\n",
        },
    ];

    for test_case in test_cases {
        let text = render_failed_list(&test_case.failed, test_case.verbose, PLAIN);
        assert_eq!(text, test_case.expected_text, "{}", test_case.description);
    }
}

#[test]
fn given_counts_when_completing_then_prints_details_and_the_summary_last() {
    let test_cases = [
        CompletionTestCase {
            description: "every item passing completes successfully",
            counts: RunCounts {
                pass: 3,
                total: 3,
                ..RunCounts::default()
            },
            details: vec!["3 local".to_owned()],
            expected_text: "\n  3 local\n✓ Completed successfully  PASS=3  FAIL=0  TOTAL=3  (1.50s)\n",
        },
        CompletionTestCase {
            description: "a failure or an item not run completes with errors",
            counts: RunCounts {
                pass: 2,
                fail: 1,
                timeout: 1,
                not_run: 2,
                total: 6,
            },
            details: vec![
                "3 local, 1 on the warehouse, 2 can't run locally".to_owned(),
                "12 statements".to_owned(),
            ],
            expected_text: "\n  3 local, 1 on the warehouse, 2 can't run locally\n  12 statements\n✗ Completed with errors  PASS=2  FAIL=1  TIMEOUT=1  NOT_RUN=2  TOTAL=6  (1.50s)\n",
        },
    ];

    for test_case in test_cases {
        let text = render_run_completion(&test_case.counts, &test_case.details, 1.5, PLAIN);
        assert_eq!(text, test_case.expected_text, "{}", test_case.description);
    }
}

#[test]
fn given_run_on_split_when_rendering_header_then_explains_what_cant_run_locally() {
    let test_cases = [
        HeaderTestCase {
            description: "everything local is one line",
            header: header(false, 0, false),
            columns: 80,
            expected_text: "Tests ready  40 selected, 40 local\n",
        },
        HeaderTestCase {
            description: "--run-on local draws the tree and caps the causes",
            header: header(true, 6, false),
            columns: 80,
            expected_text: "Tests ready  40 selected, 19 local, 21 can't run locally\n\nRun on (40)\n├── local                    19   DuckDB\n└── can't run locally        21   6 causes, listed after the run\n    ├── fn:FUNCTION_0         6   SQ0002 · 1 model\n    ├── fn:FUNCTION_1         5   SQ0002 · 1 model\n    ├── fn:FUNCTION_2         4   SQ0002 · 1 model\n    ├── fn:FUNCTION_3         3   SQ0002 · 1 model\n    ├── fn:FUNCTION_4         2   SQ0002 · 1 model\n    └── ... and 1 more\n",
        },
        HeaderTestCase {
            description: "auto at 80 columns wraps the note",
            header: header(false, 6, false),
            columns: 80,
            expected_text: "Tests ready  40 selected, 19 local, 21 on the warehouse\n  note: 21 tests run on the warehouse: fn:FUNCTION_0 6, fn:FUNCTION_1 5,\n        fn:FUNCTION_2 4, fn:FUNCTION_3 3, fn:FUNCTION_4 2, +1 more (--explain\n        for where)\n",
        },
        HeaderTestCase {
            description: "auto at 160 columns keeps the note on one line",
            header: header(false, 6, false),
            columns: 160,
            expected_text: "Tests ready  40 selected, 19 local, 21 on the warehouse\n  note: 21 tests run on the warehouse: fn:FUNCTION_0 6, fn:FUNCTION_1 5, fn:FUNCTION_2 4, fn:FUNCTION_3 3, fn:FUNCTION_4 2, +1 more (--explain for where)\n",
        },
        HeaderTestCase {
            description: "--explain adds where each cause is",
            header: header(false, 2, true),
            columns: 160,
            expected_text: "Tests ready  40 selected, 29 local, 11 on the warehouse\n  note: 11 tests run on the warehouse: fn:FUNCTION_0 6, fn:FUNCTION_1 5 (--explain for where)\n\nOn the warehouse (11 tests)\n  [ 6] fn:FUNCTION_0  sql/models/staging/stg_orders.sql:14\n  [ 5] fn:FUNCTION_1  macro cents_to_dollars\n",
        },
    ];

    for test_case in test_cases {
        let text = render_run_header(&test_case.header, test_case.columns, PLAIN);
        assert_eq!(text, test_case.expected_text, "{}", test_case.description);
    }
}

#[test]
fn given_text_when_cutting_middle_then_keeps_both_ends() {
    let test_cases = [
        CutTestCase {
            description: "text that fits is unchanged",
            text: "stg_orders",
            width: 20,
            expected_text: "stg_orders",
        },
        CutTestCase {
            description: "longer text keeps its start and end",
            text: "fct_orders__totals_match_line_items",
            width: 20,
            expected_text: "fct_orde...ine_items",
        },
        CutTestCase {
            description: "a width no wider than the ellipsis keeps the start",
            text: "fct_orders",
            width: 3,
            expected_text: "fct",
        },
    ];

    for test_case in test_cases {
        let text = cut_middle(test_case.text, test_case.width);
        assert_eq!(text, test_case.expected_text, "{}", test_case.description);
    }
}

#[test]
fn given_error_when_shortening_then_names_engine_kind_or_first_clause() {
    let test_cases = [
        ReasonTestCase {
            description: "an engine error gives its kind",
            code: None,
            message: "Binder Error: Referenced column \"amount\" not found",
            expected_reason: "Binder Error",
        },
        ReasonTestCase {
            description: "a coded message gives its code and first clause",
            code: Some("T2001"),
            message: "Expected rows differ. 2 rows only expected.",
            expected_reason: "[T2001] Expected rows differ",
        },
        ReasonTestCase {
            description: "a long clause is cut with an ellipsis",
            code: None,
            message: "the assertion returned rows for every order placed before the cutoff date",
            expected_reason: "the assertion returned rows for every order p...",
        },
    ];

    for test_case in test_cases {
        let reason = short_reason(test_case.code, test_case.message);
        assert_eq!(
            reason, test_case.expected_reason,
            "{}",
            test_case.description
        );
    }
}
