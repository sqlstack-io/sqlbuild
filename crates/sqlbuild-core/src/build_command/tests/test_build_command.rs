use crate::build_command::main::build_exit_code::build_exit_code;
use crate::build_command::main::duration_limit_error::duration_limit_error;
use crate::build_command::main::python_node_failed::python_node_failed;
use crate::build_command::main::render_confirmation_prompt::render_confirmation_prompt;
use crate::build_command::main::render_phase_timings::render_phase_timings;
use crate::build_command::main::render_python_node_rows::render_python_node_rows;
use crate::build_command::main::render_run_context::render_run_context;
use crate::build_command::models::{
    Confirmation, DurationLimit, PhaseTimings, PythonNodeRow, RunContextFacts, SelectorFileFact,
};
use crate::build_command::tests::test_types::{
    ContextTestCase, DurationTestCase, ExitCodeTestCase, PromptTestCase, PythonRowsTestCase,
    TimingsTestCase,
};
use crate::cli_text::models::CliStyle;

const PLAIN: CliStyle = CliStyle { use_color: false };

#[test]
fn given_target_duration_when_checking_adapter_then_refuses_unenforceable_limits() {
    let test_cases = [
        DurationTestCase {
            description: "an adapter without cancellation refuses max_duration",
            limit: DurationLimit {
                adapter_name: "duckdb".to_owned(),
                configures_max_duration: true,
                max_duration_seconds: Some(300),
                ..DurationLimit::default()
            },
            expected_error: Some((
                "C415",
                "Target 'default' configures max_duration, but adapter 'duckdb' cannot cancel active statements safely",
            )),
        },
        DurationTestCase {
            description: "a duration above the adapter maximum is refused",
            limit: DurationLimit {
                target_name: Some("prod".to_owned()),
                adapter_name: "warehouse".to_owned(),
                configures_max_duration: true,
                max_duration_seconds: Some(700),
                adapter_limit_seconds: Some(600),
                ..DurationLimit::default()
            },
            expected_error: Some((
                "C416",
                "Target 'prod' max_duration exceeds the largest duration supported by adapter 'warehouse' (600 seconds)",
            )),
        },
        DurationTestCase {
            description: "a duration within the adapter maximum passes",
            limit: DurationLimit {
                adapter_name: "warehouse".to_owned(),
                configures_max_duration: true,
                max_duration_seconds: Some(600),
                adapter_limit_seconds: Some(600),
                ..DurationLimit::default()
            },
            expected_error: None,
        },
        DurationTestCase {
            description: "no duration passes on any adapter",
            limit: DurationLimit {
                adapter_name: "duckdb".to_owned(),
                ..DurationLimit::default()
            },
            expected_error: None,
        },
    ];

    for test_case in test_cases {
        let actual: Option<(String, String)> = duration_limit_error(&test_case.limit)
            .map(|error| (error.code.unwrap_or_default(), error.message));
        assert_eq!(
            actual,
            test_case
                .expected_error
                .map(|(code, message)| (code.to_owned(), message.to_owned())),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_phase_timings_when_rendering_then_lists_only_measured_phases() {
    let test_cases = [
        TimingsTestCase {
            description: "measured phases keep their order",
            timings: PhaseTimings {
                compile_seconds: Some(1.234),
                execution_seconds: Some(0.005),
                total_seconds: Some(12.0),
                ..PhaseTimings::default()
            },
            expected_text: "\nPhase timings\n  compile                  1.23s\n  execution                0.01s\n  total                    12.00s\n",
        },
        TimingsTestCase {
            description: "no measured phase keeps the header",
            timings: PhaseTimings::default(),
            expected_text: "\nPhase timings\n",
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            render_phase_timings(&test_case.timings, PLAIN),
            test_case.expected_text,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_run_context_when_rendering_then_sanitizes_and_groups_counts() {
    let test_cases = [
        ContextTestCase {
            description: "unset values and both dates",
            facts: RunContextFacts {
                command: "sqb build".to_owned(),
                run_id: Some("run\u{1b}1".to_owned()),
                concurrency: 4,
                selected_count: 1200,
                total_count: 1_234_567,
                date_vars: (Some("2026-01-01".to_owned()), Some("2026-01-31".to_owned())),
                ..RunContextFacts::default()
            },
            expected_text: "Execution\n  command      sqb build\n  run_id       run\\x1b1\n  target       not set\n  database     not set\n  schema       not set\n  warehouse    not set\n  concurrency  4 configured limit\n  full_refresh false\n  selected     1,200 of 1,234,567 build resources\n  date vars    2026-01-01 to 2026-01-31\n\n",
        },
        ContextTestCase {
            description: "a warehouse, one date and selector files",
            facts: RunContextFacts {
                command: "sqb build".to_owned(),
                target: Some("dev".to_owned()),
                warehouse: Some(("small".to_owned(), "connection warehouse".to_owned())),
                full_refresh: true,
                date_vars: (None, Some("2026-01-31".to_owned())),
                selector_files: vec![
                    SelectorFileFact {
                        path: "a.txt".to_owned(),
                        selector_count: 1,
                    },
                    SelectorFileFact {
                        path: "b.txt".to_owned(),
                        selector_count: 2,
                    },
                ],
                ..RunContextFacts::default()
            },
            expected_text: "Execution\n  command      sqb build\n  run_id       not set\n  target       dev\n  database     not set\n  schema       not set\n  warehouse    small (connection warehouse)\n  concurrency  0 configured limit\n  full_refresh true\n  selected     0 of 0 build resources\n  date vars    end_date=2026-01-31\n\nSelection files\n  selector_file a.txt (1 selector)\n  selector_file b.txt (2 selectors)\n\n",
        },
        ContextTestCase {
            description: "named jobs whose nodes run on their own warehouses",
            facts: RunContextFacts {
                command: "sqb build".to_owned(),
                warehouse: Some(("small".to_owned(), "connection warehouse".to_owned())),
                job_warehouses: vec![
                    ("orders_job".to_owned(), "ORDERS_WH".to_owned()),
                    ("refunds_job".to_owned(), "REFUNDS_WH".to_owned()),
                ],
                ..RunContextFacts::default()
            },
            expected_text: "Execution\n  command      sqb build\n  run_id       not set\n  target       not set\n  database     not set\n  schema       not set\n  warehouse    small (connection warehouse); nodes of job orders_job on ORDERS_WH; nodes of job refunds_job on REFUNDS_WH\n  concurrency  0 configured limit\n  full_refresh false\n  selected     0 of 0 build resources\n  date vars    not set\n\n",
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            render_run_context(&test_case.facts, PLAIN),
            test_case.expected_text,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_python_node_results_when_rendering_then_shows_status_and_detail() {
    let test_cases = [
        PythonRowsTestCase {
            description: "success, skip reason and failure message",
            rows: vec![
                PythonNodeRow {
                    kind: "task".to_owned(),
                    node_name: "count_orders".to_owned(),
                    status: "success".to_owned(),
                    ..PythonNodeRow::default()
                },
                PythonNodeRow {
                    kind: "asset".to_owned(),
                    node_name: "extract".to_owned(),
                    status: "skipped".to_owned(),
                    skip_reason: Some("upstream failed".to_owned()),
                    ..PythonNodeRow::default()
                },
                PythonNodeRow {
                    kind: "task".to_owned(),
                    node_name: "boom".to_owned(),
                    status: "failed".to_owned(),
                    error_message: Some("boom".to_owned()),
                    skip_reason: Some("ignored".to_owned()),
                },
            ],
            expected_failed: true,
            expected_text: concat!(
                "  python    task      count_orders                                       OK\n",
                "  python    asset     extract                                            SKIP  upstream failed\n",
                "  python    task      boom                                               FAIL  boom\n",
            ),
        },
        PythonRowsTestCase {
            description: "no results write nothing",
            rows: Vec::new(),
            expected_failed: false,
            expected_text: "",
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            render_python_node_rows(&test_case.rows, PLAIN),
            test_case.expected_text,
            "{}",
            test_case.description
        );
        assert_eq!(
            test_case
                .rows
                .iter()
                .any(|row| python_node_failed(&row.status)),
            test_case.expected_failed,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_build_outcome_when_resolving_exit_code_then_any_failure_is_one() {
    let test_cases = [
        ExitCodeTestCase {
            description: "a clean build exits 0",
            outcome: (true, false, false),
            expected_exit_code: 0,
        },
        ExitCodeTestCase {
            description: "a failed build exits 1",
            outcome: (false, false, false),
            expected_exit_code: 1,
        },
        ExitCodeTestCase {
            description: "a failed Python node exits 1",
            outcome: (true, true, false),
            expected_exit_code: 1,
        },
        ExitCodeTestCase {
            description: "a failed check exits 1",
            outcome: (true, false, true),
            expected_exit_code: 1,
        },
    ];

    for test_case in test_cases {
        let (succeeded, python_failed, checks_failed) = test_case.outcome;
        assert_eq!(
            build_exit_code(succeeded, (python_failed, checks_failed)),
            test_case.expected_exit_code,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_confirmation_when_rendering_prompt_then_asks_for_expected_text() {
    let test_cases = [PromptTestCase {
        description: "the warning precedes the typed prompt",
        confirmation: Confirmation {
            warning: "Careful.".to_owned(),
            expected: "do it".to_owned(),
            ..Confirmation::default()
        },
        expected_text: "Careful.\n\nType `do it` to continue: ",
    }];

    for test_case in test_cases {
        assert_eq!(
            render_confirmation_prompt(&test_case.confirmation),
            test_case.expected_text,
            "{}",
            test_case.description
        );
    }
}
