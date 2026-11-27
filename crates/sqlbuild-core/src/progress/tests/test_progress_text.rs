use crate::cli_text::models::CliStyle;
use crate::progress::main::connection_phase_line::connection_phase_line;
use crate::progress::main::connection_start_message::connection_start_message;
use crate::progress::main::execution_header_text::execution_header_text;
use crate::progress::main::format_expectation_detail::format_expectation_detail;
use crate::progress::main::format_expectation_name::format_expectation_name;
use crate::progress::main::format_inspection_record::format_inspection_record;
use crate::progress::main::format_status_spinner_line::format_status_spinner_line;
use crate::progress::main::is_planning_completion_message::is_planning_completion_message;
use crate::progress::main::is_planning_failure_message::is_planning_failure_message;
use crate::progress::main::machine_output_mode::machine_output_mode;
use crate::progress::models::{ExpectationStep, InspectionRecord, OutputModeFacts};

const PLAIN: CliStyle = CliStyle { use_color: false };
use crate::progress::tests::test_types::{
    CompletionMessageTestCase, ExpectationDetailTestCase, InspectionRecordTestCase,
    OutputModeTestCase, ProgressLineTestCase, StatusSpinnerLineTestCase,
};

#[test]
fn given_planning_message_when_classifying_then_completions_are_recognized() {
    let test_cases = [
        CompletionMessageTestCase {
            description: "an applied clone plan completes",
            message: "Applied clone plan. (0.10s)",
            expected_is_completion: true,
        },
        CompletionMessageTestCase {
            description: "a prepared clone plan completes",
            message: "Prepared clone plan. (0.10s)",
            expected_is_completion: true,
        },
        CompletionMessageTestCase {
            description: "an ongoing clone preparation does not complete",
            message: "Preparing clone plan...",
            expected_is_completion: false,
        },
        CompletionMessageTestCase {
            description: "a finalized dbt run completes",
            message: "Finalized dbt run.",
            expected_is_completion: true,
        },
        CompletionMessageTestCase {
            description: "evaluated rules complete",
            message: "Evaluated rules. (built-in 0.00s, custom 0.00s)",
            expected_is_completion: true,
        },
        CompletionMessageTestCase {
            description: "an ongoing phase does not complete",
            message: "Compiling project...",
            expected_is_completion: false,
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            is_planning_completion_message(test_case.message),
            test_case.expected_is_completion,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_planning_message_when_classifying_then_failures_are_recognized() {
    let test_cases = [
        CompletionMessageTestCase {
            description: "a failed cursor watermark ends its phase",
            message: "Failed runtime cursor watermark (1/1): dev.orders.updated_at [max] (0.10s): boom",
            expected_is_completion: true,
        },
        CompletionMessageTestCase {
            description: "a successful completion is not a failure",
            message: "Inspected runtime cursor watermark (1/1): dev.orders.updated_at [max] (0.10s)",
            expected_is_completion: false,
        },
        CompletionMessageTestCase {
            description: "an ongoing phase is not a failure",
            message: "Inspecting runtime cursor watermark (1/1): dev.orders.updated_at [max]...",
            expected_is_completion: false,
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            is_planning_failure_message(test_case.message),
            test_case.expected_is_completion,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_connection_and_header_when_formatting_then_lines_match() {
    let test_cases = [
        ProgressLineTestCase {
            description: "a single connection omits its count",
            line: connection_start_message("duckdb", 1),
            expected_line: "Connecting to duckdb...",
        },
        ProgressLineTestCase {
            description: "several connections show their count",
            line: connection_start_message("databricks", 8),
            expected_line: "Connecting to databricks (8 connections)...",
        },
        ProgressLineTestCase {
            description: "a connected warehouse reports its elapsed time",
            line: connection_phase_line(PLAIN, true, ("duckdb", 0.034)),
            expected_line: "\u{2713} Warehouse connected  duckdb  (0.03s)",
        },
        ProgressLineTestCase {
            description: "a failed connection reports the time it took",
            line: connection_phase_line(PLAIN, false, ("duckdb", 18.424)),
            expected_line: "\u{2717} Warehouse connection failed  duckdb  (after 18.42s)",
        },
        ProgressLineTestCase {
            description: "a header with a target lists it before concurrency",
            line: execution_header_text(PLAIN, ("sqb build", Some("dev"), 4), None),
            expected_line: "Execution  sqb build  (target: dev, concurrency: 4)\n\n",
        },
        ProgressLineTestCase {
            description: "a header without a target lists only concurrency",
            line: execution_header_text(PLAIN, ("sqb test", None, 1), None),
            expected_line: "Execution  sqb test  (concurrency: 1)\n\n",
        },
        ProgressLineTestCase {
            description: "a header with engines lists them after concurrency",
            line: execution_header_text(PLAIN, ("sqb test", None, 8), Some("DuckDB and snowflake")),
            expected_line: "Execution  sqb test  (concurrency: 8, DuckDB and snowflake)\n\n",
        },
        ProgressLineTestCase {
            description: "a compared model reads as expected",
            line: format_expectation_name("orders"),
            expected_line: "expected orders",
        },
        ProgressLineTestCase {
            description: "an assertion keeps its name",
            line: format_expectation_name("assertion totals_are_positive"),
            expected_line: "assertion totals_are_positive",
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            test_case.line, test_case.expected_line,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_expectation_step_when_formatting_detail_then_detail_matches() {
    let failing: ExpectationStep = ExpectationStep {
        model_name: "orders".to_owned(),
        outcome: "fail".to_owned(),
        actual_row_count: 2,
        expected_row_count: 3,
        ..ExpectationStep::default()
    };
    let test_cases = [
        ExpectationDetailTestCase {
            description: "a passing step has no detail",
            step: ExpectationStep {
                outcome: "pass".to_owned(),
                ..failing.clone()
            },
            expected_detail: "",
        },
        ExpectationDetailTestCase {
            description: "an assertion reports its row count",
            step: ExpectationStep {
                model_name: "assertion totals".to_owned(),
                actual_row_count: 1,
                ..failing.clone()
            },
            expected_detail: "  1 row",
        },
        ExpectationDetailTestCase {
            description: "a step without set differences reports mismatched rows",
            step: ExpectationStep {
                mismatched_row_count: 4,
                ..failing.clone()
            },
            expected_detail: "  4 mismatched",
        },
        ExpectationDetailTestCase {
            description: "set differences list counts, samples and columns",
            step: ExpectationStep {
                unexpected_row_count: Some(1),
                missing_samples: vec![vec![("id".to_owned(), "7".to_owned())]],
                column_differences: vec![("total".to_owned(), "1".to_owned(), "2".to_owned())],
                ..failing.clone()
            },
            expected_detail: "  unexpected=1; missing=None; row counts actual=2 expected=3; \
                missing sample 1: id=7; differing column total: actual=1, expected=2",
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            format_expectation_detail(&test_case.step),
            test_case.expected_detail,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_inspection_record_when_formatting_then_sql_is_collapsed_and_cut() {
    let test_cases = [
        InspectionRecordTestCase {
            description: "a read reports its row count on one line",
            record: InspectionRecord {
                sql: "SELECT 1\n  FROM orders".to_owned(),
                elapsed_seconds: 0.125,
                row_count: Some(1),
                error: None,
            },
            expected_line: "  inspect   0.12s  1 row  SELECT 1 FROM orders".to_owned(),
        },
        InspectionRecordTestCase {
            description: "a failed read reports its error and long SQL is cut",
            record: InspectionRecord {
                sql: "x".repeat(200),
                elapsed_seconds: 12.5,
                row_count: None,
                error: Some("no\nsuch table".to_owned()),
            },
            expected_line: format!(
                "  inspect  12.50s  failed: no such table  {}...",
                "x".repeat(157)
            ),
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            format_inspection_record(&test_case.record),
            test_case.expected_line,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_parsed_options_when_choosing_output_mode_then_machine_output_is_detected() {
    let facts = |command: &str, dbt_args: &[&str]| OutputModeFacts {
        command: Some(command.to_owned()),
        dbt_args: dbt_args.iter().map(|arg| (*arg).to_owned()).collect(),
        ..OutputModeFacts::default()
    };
    let test_cases = [
        OutputModeTestCase {
            description: "dbt plan with --json",
            facts: facts("dbt", &["--select", "orders", "--json"]),
            expected_machine_output: true,
        },
        OutputModeTestCase {
            description: "dbt plan without --json",
            facts: facts("dbt", &["--select", "orders"]),
            expected_machine_output: false,
        },
        OutputModeTestCase {
            description: "plan --json",
            facts: OutputModeFacts {
                json: true,
                ..facts("plan", &[])
            },
            expected_machine_output: true,
        },
        OutputModeTestCase {
            description: "query json",
            facts: OutputModeFacts {
                query_format: Some("json".to_owned()),
                ..facts("query", &[])
            },
            expected_machine_output: true,
        },
        OutputModeTestCase {
            description: "lineage text",
            facts: OutputModeFacts {
                lineage_format: Some("text".to_owned()),
                ..facts("lineage", &[])
            },
            expected_machine_output: false,
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            machine_output_mode(&test_case.facts),
            test_case.expected_machine_output,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_status_message_when_rendering_spinner_line_then_fits_terminal() {
    let test_cases = [
        StatusSpinnerLineTestCase {
            description: "short message is kept with a green frame",
            message: "Compiling project graph...",
            columns: 80,
            style: CliStyle { use_color: true },
            expected_line: "\x1b[32m\u{280b}\x1b[0m Compiling project graph...",
        },
        StatusSpinnerLineTestCase {
            description: "long message is cut below the terminal width",
            message: "Compiling project graph...",
            columns: 15,
            style: CliStyle { use_color: false },
            expected_line: "\u{280b} Compiling...",
        },
        StatusSpinnerLineTestCase {
            description: "unknown width keeps only the first line",
            message: "first line\nsecond line",
            columns: 0,
            style: CliStyle { use_color: false },
            expected_line: "\u{280b} first line",
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            format_status_spinner_line(
                "\u{280b}",
                test_case.message,
                test_case.columns,
                test_case.style
            ),
            test_case.expected_line,
            "{}",
            test_case.description
        );
    }
}
