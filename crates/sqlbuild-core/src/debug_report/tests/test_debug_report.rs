use crate::cli_text::models::CliStyle;
use crate::debug_report::main::debug_connection_lines::debug_connection_lines;
use crate::debug_report::main::debug_failure::debug_failure;
use crate::debug_report::main::format_debug_json::format_debug_json;
use crate::debug_report::main::format_debug_text::format_debug_text;
use crate::debug_report::models::{
    ConnectionCheck, ConnectionValue, DebugFailure, DebugLine, DebugReport, DebugStatus,
};
use crate::debug_report::tests::helpers::{line_summary, sample_report, text};
use crate::debug_report::tests::test_types::{
    ConnectionLinesTestCase, DebugFailureTestCase, FormatTestCase,
};

/// Expectations recorded from the retired Python formatter.
#[test]
fn given_debug_report_when_formatting_then_matches_python_outputs() {
    let test_cases = [FormatTestCase {
        description: "renders diagnostics sections and json sections",
        report: sample_report(),
        expected_text: "\nSQLBuild Diagnostics\n\nRuntime:\n  sqlbuild version: 0.2.1\n\nConfiguration:\n  project file: /repo/sqlbuild_project.toml [OK found and valid]\n\nProviders:\n  providers: 1 [OK discovered]\n  marker_provider: providers/marker.py:MarkerProvider [OK valid settings]\n\nConnection:\n  token: ****\n  connection test: [ERROR authentication failed]\n  query test: [SKIP connection failed]\n\n",
        expected_json: r#"{"configuration": [{"label": "project file", "message": "/repo/sqlbuild_project.toml", "status": "OK", "status_message": "found and valid"}], "connection": [{"label": "token", "message": "****"}, {"label": "connection test", "message": "", "status": "ERROR", "status_message": "authentication failed"}, {"label": "query test", "message": "", "status": "SKIP", "status_message": "connection failed"}], "local_files": ["/repo/warehouse.db"], "providers": [{"label": "providers", "message": "1", "status": "OK", "status_message": "discovered"}, {"label": "marker_provider", "message": "providers/marker.py:MarkerProvider", "status": "OK", "status_message": "valid settings"}], "runtime": [{"label": "sqlbuild version", "message": "0.2.1"}], "success": false}"#,
        expected_color_fragments: &[
            "\x1b[34m\x1b[1mSQLBuild Diagnostics\x1b[0m",
            "\x1b[1mRuntime:\x1b[0m",
            "\x1b[32m[OK found and valid]\x1b[0m",
            "\x1b[38;5;167m[ERROR authentication failed]\x1b[0m",
            "\x1b[2m[SKIP connection failed]\x1b[0m",
        ],
    }];

    for test_case in test_cases {
        let plain: String = format_debug_text(&test_case.report, CliStyle { use_color: false });
        let colored: String = format_debug_text(&test_case.report, CliStyle { use_color: true });
        assert_eq!(plain, test_case.expected_text, "{}", test_case.description);
        assert_eq!(
            format_debug_json(&test_case.report),
            test_case.expected_json,
            "{}",
            test_case.description
        );
        for fragment in test_case.expected_color_fragments {
            assert!(
                colored.contains(fragment),
                "{}: {fragment:?}",
                test_case.description
            );
        }
    }
}

#[test]
fn given_connection_settings_when_listing_then_values_are_sorted_masked_and_shortened() {
    let test_cases = [
        ConnectionLinesTestCase {
            description: "secrets mask, long text shortens and scalars stay whole",
            settings: vec![
                (
                    "user".to_owned(),
                    text("a_very_long_user_name_exceeding_32_chars"),
                ),
                ("Password".to_owned(), text("hunter2")),
                (
                    "port".to_owned(),
                    ConnectionValue::Scalar("12345678901234567890123456789012345".to_owned()),
                ),
                ("role".to_owned(), ConnectionValue::Null),
                (
                    "account".to_owned(),
                    text("ünïcödé-ünïcödé-ünïcödé-ünïcödé-x"),
                ),
            ],
            check: ConnectionCheck::Connected {
                query_error: Some("boom".to_owned()),
            },
            expected_lines: vec![
                ("Password", "****", None),
                ("account", "ünïc...dé-x", None),
                ("port", "12345678901234567890123456789012345", None),
                ("role", "null", None),
                ("user", "a_ve...hars", None),
                ("connection test", "", Some("connected")),
                ("query test", "", Some("boom")),
            ],
        },
        ConnectionLinesTestCase {
            description: "no settings and a failed connection",
            settings: Vec::new(),
            check: ConnectionCheck::ConnectFailed("refused".to_owned()),
            expected_lines: vec![
                ("settings", "no keys", None),
                ("connection test", "", Some("refused")),
                ("query test", "", Some("connection failed")),
            ],
        },
    ];

    for test_case in test_cases {
        let lines: Vec<DebugLine> = debug_connection_lines(&test_case.settings, &test_case.check);
        let summaries: Vec<(&str, &str, Option<&str>)> = lines.iter().map(line_summary).collect();
        assert_eq!(
            summaries, test_case.expected_lines,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_debug_report_when_its_failure_is_read_then_the_first_failed_check_is_coded() {
    let test_cases = [
        DebugFailureTestCase {
            description: "a failed connection test is a connection error",
            report: sample_report(),
            expected_failure: Some(DebugFailure {
                error_type: "ConnectionError".to_owned(),
                error_code: "C980".to_owned(),
                message: "connection test: authentication failed".to_owned(),
            }),
        },
        DebugFailureTestCase {
            description: "another failed check is a configuration error",
            report: DebugReport {
                configuration: vec![DebugLine::checked(
                    "project file",
                    "/repo/sqlbuild_project.toml",
                    (DebugStatus::Error, Some("invalid TOML")),
                )],
                ..DebugReport::default()
            },
            expected_failure: Some(DebugFailure {
                error_type: "ConfigurationError".to_owned(),
                error_code: "C982".to_owned(),
                message: "project file: invalid TOML".to_owned(),
            }),
        },
        DebugFailureTestCase {
            description: "a passing report has no failure",
            report: DebugReport {
                connection: vec![DebugLine::checked(
                    "connection test",
                    "",
                    (DebugStatus::Ok, Some("connected")),
                )],
                ..DebugReport::default()
            },
            expected_failure: None,
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            debug_failure(&test_case.report),
            test_case.expected_failure,
            "{}",
            test_case.description
        );
    }
}
