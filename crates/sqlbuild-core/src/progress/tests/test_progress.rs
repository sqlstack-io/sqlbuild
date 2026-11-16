use std::collections::BTreeMap;

use crate::progress::main::consume_progress_event::consume_progress_event;
use crate::progress::main::format_statement_summary::format_statement_summary;
use crate::progress::main::statement_summary_json::statement_summary_json;
use crate::progress::models::{
    ProgressEvent, ProgressPayload, ProgressProjection, ProgressSettings, StatementSummary,
    StatementTiming, StatementTimings,
};
use crate::progress::tests::helpers::{
    attempt_event, audit_rows, operation_event, orders_claim, plain, project_case, statement_event,
};
use crate::progress::tests::test_types::{
    AuditRowTestCase, ProjectionTestCase, StatementCountTestCase, StatementSummaryTextTestCase,
};

#[test]
fn given_lifecycle_facts_when_projecting_then_progress_lines_match() {
    let test_cases = [
        ProjectionTestCase {
            description: "an unenriched attempt renders its start and terminal once",
            settings: plain(),
            enriched: vec![],
            events: vec![
                attempt_event("e1", "resource_attempt_started", ("a1", None)),
                attempt_event("e2", "resource_attempt_completed", ("a1", Some(1234.0))),
                attempt_event("e2", "resource_attempt_completed", ("a1", Some(1234.0))),
            ],
            claims: vec![],
            expected_lines: vec!["  table     orders START", "  table     orders OK 1.23s"],
            expected_durations: vec![],
        },
        ProjectionTestCase {
            description: "an enriched terminal is claimed and earlier attempts render first",
            settings: plain(),
            enriched: vec!["orders"],
            events: vec![
                attempt_event("e1", "resource_attempt_failed", ("a1", Some(10.0))),
                attempt_event("e2", "resource_attempt_completed", ("a2", Some(20.0))),
            ],
            claims: vec![orders_claim(), orders_claim()],
            expected_lines: vec!["  table     orders FAIL 0.01s"],
            expected_durations: vec![Some(20.0), None],
        },
        ProjectionTestCase {
            description: "an unclaimed enriched terminal renders on close",
            settings: plain(),
            enriched: vec!["orders"],
            events: vec![attempt_event(
                "e1",
                "resource_attempt_skipped",
                ("a1", None),
            )],
            claims: vec![],
            expected_lines: vec!["  table     orders SKIP"],
            expected_durations: vec![],
        },
        ProjectionTestCase {
            description: "a terminal stream leaves starts to live rows",
            settings: ProgressSettings {
                is_tty: true,
                ..plain()
            },
            enriched: vec![],
            events: vec![
                attempt_event("e1", "resource_attempt_started", ("a1", None)),
                operation_event("o1", "operation_started", ("project_compile", None)),
                operation_event("o2", "operation_completed", ("project_compile", None)),
            ],
            claims: vec![],
            expected_lines: vec![],
            expected_durations: vec![],
        },
        ProjectionTestCase {
            description: "a top-level operation renders its start and terminal",
            settings: plain(),
            enriched: vec![],
            events: vec![
                operation_event("o1", "operation_started", ("project_compile", None)),
                operation_event("o2", "operation_failed", ("project_compile", None)),
            ],
            claims: vec![],
            expected_lines: vec!["Project compile  START", "Project compile  FAIL  (1.50s)"],
            expected_durations: vec![],
        },
        ProjectionTestCase {
            description: "a quiet resource operation renders only its failure",
            settings: plain(),
            enriched: vec!["orders"],
            events: vec![
                attempt_event("e1", "resource_attempt_started", ("a1", None)),
                operation_event("o1", "operation_started", ("staging_creation", Some("a1"))),
                operation_event("o2", "operation_failed", ("staging_creation", Some("a1"))),
            ],
            claims: vec![],
            expected_lines: vec![
                "  table     orders START",
                "    Create staging relation  FAIL  (1.50s)",
            ],
            expected_durations: vec![],
        },
        ProjectionTestCase {
            description: "freshness metadata and retries render their details",
            settings: plain(),
            enriched: vec![],
            events: vec![
                operation_event(
                    "o1",
                    "operation_started",
                    ("source_freshness_metadata_observation", None),
                ),
                ProgressEvent {
                    payload: ProgressPayload {
                        metadata_item_count: Some(1),
                        metadata_unknown_count: Some(1),
                        ..operation_event(
                            "o2",
                            "operation_completed",
                            ("source_freshness_metadata_observation", None),
                        )
                        .payload
                    },
                    ..operation_event(
                        "o2",
                        "operation_completed",
                        ("source_freshness_metadata_observation", None),
                    )
                },
                ProgressEvent {
                    payload: ProgressPayload {
                        failed_attempt_number: Some(1),
                        next_attempt_number: Some(2),
                        delay_ms: Some(250.0),
                        ..ProgressPayload::default()
                    },
                    ..attempt_event("r1", "retry_scheduled", ("a1", None))
                },
            ],
            claims: vec![],
            expected_lines: vec![
                "Freshness metadata  START",
                "Freshness metadata  OK (1 source, 1 unknown)  (1.50s)",
                "    retry 1->2 in 0.25s",
            ],
            expected_durations: vec![],
        },
        ProjectionTestCase {
            description: "debug statements render start, query id, heartbeat and failure",
            settings: ProgressSettings {
                debug: true,
                ..plain()
            },
            enriched: vec![],
            events: vec![
                attempt_event("e1", "resource_attempt_started", ("a1", None)),
                ProgressEvent {
                    statement_id: Some("s1".to_owned()),
                    payload: ProgressPayload {
                        intent: "execute".to_owned(),
                        statement_kind: "CREATE".to_owned(),
                        ..ProgressPayload::default()
                    },
                    ..attempt_event("s-start", "statement_started", ("a1", None))
                },
                ProgressEvent {
                    statement_id: Some("s1".to_owned()),
                    payload: ProgressPayload {
                        query_id: Some("q1".to_owned()),
                        intent: "execute".to_owned(),
                        statement_kind: "CREATE".to_owned(),
                        ..ProgressPayload::default()
                    },
                    ..attempt_event("s-sub", "statement_submitted", ("a1", None))
                },
                ProgressEvent {
                    statement_id: Some("s1".to_owned()),
                    payload: ProgressPayload {
                        duration_ms: Some(31000.0),
                        intent: "execute".to_owned(),
                        statement_kind: "CREATE".to_owned(),
                        ..ProgressPayload::default()
                    },
                    ..attempt_event("s-hb", "statement_heartbeat", ("a1", None))
                },
                ProgressEvent {
                    statement_id: Some("s1".to_owned()),
                    payload: ProgressPayload {
                        intent: "execute".to_owned(),
                        statement_kind: "CREATE".to_owned(),
                        ..ProgressPayload::default()
                    },
                    ..attempt_event("s-fail", "statement_failed", ("a1", None))
                },
            ],
            claims: vec![],
            expected_lines: vec![
                "  table     orders START",
                "    statement  model=orders  phase=execute  kind=CREATE  START",
                "    statement  model=orders  phase=execute  kind=CREATE  query_id=q1",
                "    statement  model=orders  phase=execute  kind=CREATE  RUNNING 31.0s  query_id=q1",
                "    statement  model=orders  phase=execute  kind=CREATE  FAIL  query_id=q1",
            ],
            expected_durations: vec![],
        },
    ];

    for test_case in test_cases {
        let (lines, durations) = project_case(&test_case);
        assert_eq!(
            (lines, durations),
            (
                test_case
                    .expected_lines
                    .iter()
                    .map(|line| (*line).to_owned())
                    .collect::<Vec<String>>(),
                test_case.expected_durations
            ),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_standalone_audit_when_projecting_then_row_names_target_and_outcome() {
    let test_cases = [
        AuditRowTestCase {
            description: "a seed column audit names its target and passes",
            resource_id: "audit:not_null:seed:customers:customer_id",
            terminal_event: "resource_attempt_completed",
            recorded_outcomes: vec![("a1", "pass")],
            expected_lines: vec![
                "  audit     not_null (customers.customer_id) START",
                "  audit     not_null (customers.customer_id) PASS 0.01s",
            ],
        },
        AuditRowTestCase {
            description: "a completed audit with a warn outcome shows WARN",
            resource_id: "audit:not_null:source:raw_orders:order_id",
            terminal_event: "resource_attempt_completed",
            recorded_outcomes: vec![("a1", "warn")],
            expected_lines: vec![
                "  audit     not_null (raw_orders.order_id) START",
                "  audit     not_null (raw_orders.order_id) WARN 0.01s",
            ],
        },
        AuditRowTestCase {
            description: "a failed singular audit has no target and shows FAIL",
            resource_id: "audit:not_null:singular",
            terminal_event: "resource_attempt_failed",
            recorded_outcomes: vec![("a1", "error")],
            expected_lines: vec![
                "  audit     not_null START",
                "  audit     not_null FAIL 0.01s",
            ],
        },
        AuditRowTestCase {
            description: "a completed audit without its own recorded outcome passes",
            resource_id: "audit:not_null:model:orders",
            terminal_event: "resource_attempt_completed",
            recorded_outcomes: vec![("a2", "warn")],
            expected_lines: vec![
                "  audit     not_null (orders) START",
                "  audit     not_null (orders) PASS 0.01s",
            ],
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            audit_rows(&test_case),
            test_case.expected_lines,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_statement_terminals_when_projecting_then_every_finished_statement_is_counted_once() {
    let test_cases = [
        StatementCountTestCase {
            description: "a known resource's terminal takes the query id its submission reported",
            events: vec![
                attempt_event("r1", "resource_attempt_started", ("a1", None)),
                statement_event(
                    ("s1", "statement_started"),
                    ("st1", "CREATE", None),
                    Some("a1"),
                ),
                statement_event(
                    ("s2", "statement_submitted"),
                    ("st1", "CREATE", Some("q1")),
                    Some("a1"),
                ),
                statement_event(
                    ("s3", "statement_completed"),
                    ("st1", "CREATE", None),
                    Some("a1"),
                ),
            ],
            expected_by_kind: vec![("CREATE", 1)],
            expected_query_ids: vec!["q1"],
        },
        StatementCountTestCase {
            description: "a redelivered terminal is counted once",
            events: vec![
                statement_event(
                    ("s1", "statement_completed"),
                    ("st1", "WITH", Some("q1")),
                    None,
                ),
                statement_event(
                    ("s1", "statement_completed"),
                    ("st1", "WITH", Some("q1")),
                    None,
                ),
            ],
            expected_by_kind: vec![("WITH", 1)],
            expected_query_ids: vec!["q1"],
        },
        StatementCountTestCase {
            description: "statements outside known resources count, failed ones too",
            events: vec![
                statement_event(
                    ("s1", "statement_completed"),
                    ("st1", "WITH", Some("q1")),
                    None,
                ),
                statement_event(("s2", "statement_failed"), ("st2", "DROP", None), None),
                statement_event(
                    ("s3", "statement_completed"),
                    ("st3", "WITH", Some("q3")),
                    None,
                ),
            ],
            expected_by_kind: vec![("DROP", 1), ("WITH", 2)],
            expected_query_ids: vec!["q1", "q3"],
        },
        StatementCountTestCase {
            description: "starts, submissions and heartbeats alone are not finished statements",
            events: vec![
                statement_event(("s1", "statement_started"), ("st1", "SELECT", None), None),
                statement_event(
                    ("s2", "statement_submitted"),
                    ("st1", "SELECT", Some("q1")),
                    None,
                ),
                statement_event(
                    ("s3", "statement_heartbeat"),
                    ("st1", "SELECT", Some("q1")),
                    None,
                ),
            ],
            expected_by_kind: vec![],
            expected_query_ids: vec![],
        },
    ];

    for test_case in test_cases {
        let mut projection: ProgressProjection = ProgressProjection::new(plain());
        for event in &test_case.events {
            projection = consume_progress_event(projection, event).0;
        }
        let summary: &StatementSummary = projection.statement_summary();
        assert_eq!(
            (summary.by_kind.clone(), summary.query_ids.clone()),
            (
                test_case
                    .expected_by_kind
                    .iter()
                    .map(|(kind, count)| ((*kind).to_owned(), *count))
                    .collect::<BTreeMap<String, u64>>(),
                test_case
                    .expected_query_ids
                    .iter()
                    .map(|query_id| (*query_id).to_owned())
                    .collect::<Vec<String>>(),
            ),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_statement_summary_when_formatting_then_line_and_json_report_kinds_and_timing() {
    let summary: StatementSummary = StatementSummary {
        by_kind: BTreeMap::from([
            ("CREATE".to_owned(), 2),
            ("DROP".to_owned(), 2),
            ("SELECT".to_owned(), 7),
            ("ALTER".to_owned(), 1),
        ]),
        query_ids: (0..12).map(|index| format!("q{index}")).collect(),
    };
    let reported = |compile_ms: f64, execution_ms: f64, timed_statements: u64| {
        StatementTiming::Reported(StatementTimings {
            compile_ms,
            execution_ms,
            timed_statements,
        })
    };
    let test_cases = [
        StatementSummaryTextTestCase {
            description: "kinds run from most to least frequent, ties by name",
            summary: summary.clone(),
            timing: reported(1540.0, 260.0, 12),
            expected_line: Some(
                "Statements: 12 (SELECT 7, CREATE 2, DROP 2, ALTER 1); compile 1.5s, execution 0.3s",
            ),
            expected_json: serde_json::json!({
                "total": 12,
                "by_kind": {"ALTER": 1, "CREATE": 2, "DROP": 2, "SELECT": 7},
                "compile_time": {
                    "source": "snowflake_query_history",
                    "compile_ms": 1540.0,
                    "execution_ms": 260.0,
                    "timed_statements": 12,
                    "identified_statements": 12,
                },
            }),
        },
        StatementSummaryTextTestCase {
            description: "statements missing from query history are called out",
            summary: summary.clone(),
            timing: reported(1000.0, 0.0, 10),
            expected_line: Some(
                "Statements: 12 (SELECT 7, CREATE 2, DROP 2, ALTER 1); compile 1.0s, execution 0.0s (10 of 12 statements in query history)",
            ),
            expected_json: serde_json::json!({
                "total": 12,
                "by_kind": {"ALTER": 1, "CREATE": 2, "DROP": 2, "SELECT": 7},
                "compile_time": {
                    "source": "snowflake_query_history",
                    "compile_ms": 1000.0,
                    "execution_ms": 0.0,
                    "timed_statements": 10,
                    "identified_statements": 12,
                },
            }),
        },
        StatementSummaryTextTestCase {
            description: "an adapter without query history says so",
            summary: summary.clone(),
            timing: StatementTiming::NotReported("duckdb".to_owned()),
            expected_line: Some(
                "Statements: 12 (SELECT 7, CREATE 2, DROP 2, ALTER 1); compile time not reported by duckdb",
            ),
            expected_json: serde_json::json!({
                "total": 12,
                "by_kind": {"ALTER": 1, "CREATE": 2, "DROP": 2, "SELECT": 7},
                "compile_time": null,
            }),
        },
        StatementSummaryTextTestCase {
            description: "a failed read says why",
            summary: summary.clone(),
            timing: StatementTiming::Failed("query history read failed".to_owned()),
            expected_line: Some(
                "Statements: 12 (SELECT 7, CREATE 2, DROP 2, ALTER 1); compile time unavailable (query history read failed)",
            ),
            expected_json: serde_json::json!({
                "total": 12,
                "by_kind": {"ALTER": 1, "CREATE": 2, "DROP": 2, "SELECT": 7},
                "compile_time": {
                    "source": "snowflake_query_history",
                    "error": "query history read failed",
                },
            }),
        },
        StatementSummaryTextTestCase {
            description: "a run without statements has no line",
            summary: StatementSummary::default(),
            timing: reported(0.0, 0.0, 0),
            expected_line: None,
            expected_json: serde_json::json!({
                "total": 0,
                "by_kind": {},
                "compile_time": {
                    "source": "snowflake_query_history",
                    "compile_ms": 0.0,
                    "execution_ms": 0.0,
                    "timed_statements": 0,
                    "identified_statements": 0,
                },
            }),
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            (
                format_statement_summary(&test_case.summary, &test_case.timing),
                statement_summary_json(&test_case.summary, &test_case.timing),
            ),
            (
                test_case.expected_line.map(str::to_owned),
                test_case.expected_json,
            ),
            "{}",
            test_case.description
        );
    }
}
