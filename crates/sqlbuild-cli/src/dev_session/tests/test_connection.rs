//! The foreground `sqb debug` connection: its verdict from the report, and its stderr split from the client's.

use serde_json::json;

use crate::dev_session::_helpers::connection::{connection_outcome, split_debug_output};
use crate::dev_session::constants::NO_REPORT;
use crate::dev_session::tests::helpers::{report, row, state_text};
use crate::dev_session::tests::test_types::{OutcomeTestCase, SplitTestCase};

#[test]
fn given_a_debug_report_when_read_then_the_adapter_and_warehouse_state_follow_its_rows() {
    let configured = json!([
        row("project", Some("OK"), "shop", "loaded"),
        row("adapter", Some("OK"), "duckdb", "found"),
    ]);
    let test_cases = [
        OutcomeTestCase {
            description: "a passing connection test is connected, detailed by the status-less rows",
            report: report(
                configured.clone(),
                json!([
                    row("database", None, "/work/shop/warehouse.duckdb", ""),
                    row("connection test", Some("OK"), "", "connected"),
                    row("query test", Some("OK"), "", "SELECT 1"),
                ]),
            ),
            expected_adapter: "duckdb",
            expected_state: r#"Connected { detail: "/work/shop/warehouse.duckdb" }"#.to_owned(),
        },
        OutcomeTestCase {
            description: "a failing connection test reports its own message, not the skipped query test",
            report: report(
                configured,
                json!([
                    row("database", None, "/missing/warehouse.duckdb", ""),
                    row(
                        "connection test",
                        Some("ERROR"),
                        "",
                        "IO Error: Cannot open file"
                    ),
                    row("query test", Some("SKIP"), "", "connection failed"),
                ]),
            ),
            expected_adapter: "duckdb",
            expected_state: r#"Failed { message: "IO Error: Cannot open file" }"#.to_owned(),
        },
        OutcomeTestCase {
            description: "with no connection rows, the first configuration error explains the failure",
            report: report(
                json!([
                    row("project", Some("OK"), "shop", "loaded"),
                    row(
                        "adapter",
                        Some("ERROR"),
                        "",
                        "adapter package is not installed"
                    ),
                ]),
                json!([]),
            ),
            expected_adapter: "",
            expected_state: r#"Failed { message: "adapter package is not installed" }"#.to_owned(),
        },
        OutcomeTestCase {
            description: "output that is not a report, such as an unknown target, fails without one",
            report: String::new(),
            expected_adapter: "",
            expected_state: format!("Failed {{ message: {NO_REPORT:?} }}"),
        },
    ];

    for test_case in test_cases {
        let (adapter, state) = connection_outcome(test_case.report.as_bytes());

        assert_eq!(
            adapter, test_case.expected_adapter,
            "{}",
            test_case.description
        );
        assert_eq!(
            state_text(&state),
            test_case.expected_state,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_debug_stderr_when_split_then_only_complete_sqlbuild_rows_leave_the_client_text() {
    let test_cases = [
        SplitTestCase {
            description: "a prompt without a newline stays with the client, after SQLBuild's own row",
            chunk: "Project discovery  OK  (0.02s)\nEnter passcode: ",
            expected_client: "Enter passcode: ",
            expected_rows: vec!["Project discovery  OK  (0.02s)"],
        },
        SplitTestCase {
            description: "client lines that only mention a status, or start blank, are kept as written",
            chunk: "Opening the browser for OK  sign-in\n  OK  indented\r\nConnection test  FAIL  (1.2s)\n",
            expected_client: "Opening the browser for OK  sign-in\n  OK  indented\r\n",
            expected_rows: vec!["Connection test  FAIL  (1.2s)"],
        },
        SplitTestCase {
            description: "a line cut off by the end of the chunk passes through as client text",
            chunk: "Connection test  START",
            expected_client: "Connection test  START",
            expected_rows: Vec::new(),
        },
    ];

    for test_case in test_cases {
        let (client, rows) = split_debug_output(test_case.chunk);

        assert_eq!(
            client, test_case.expected_client,
            "{}",
            test_case.description
        );
        assert_eq!(rows, test_case.expected_rows, "{}", test_case.description);
    }
}
