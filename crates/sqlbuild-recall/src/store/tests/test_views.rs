use crate::records::main::artifact_rows::artifact_rows;
use crate::records::main::delivery_row::delivery_row;
use crate::records::main::heartbeat_row::heartbeat_row;
use crate::records::main::invocation_row::invocation_row;
use crate::records::main::sha256_hex::sha256_hex;
use crate::records::models::{ArtifactInput, InvocationContext, OutputStream, RecallValue};
use crate::store::tests::helpers::{
    NOW, artifact, chunk, chunks, delivered, envelope, event, integer, projected, text, wall_clock,
};
use crate::store::tests::test_types::{PayloadTestCase, ProjectionTestCase};

const BINARY_PIECES: [(OutputStream, &[u8]); 4] = [
    (OutputStream::Stdout, b"line one\n\x00\xff"),
    (OutputStream::Stderr, b"warn \xe2\x9c\x93\n"),
    (OutputStream::Stdout, b"\xc3\x28 tail"),
    (OutputStream::Stderr, b"\x00end"),
];

const STATEMENT_SQL: &str = "select 1";

#[test]
fn given_output_with_and_without_gaps_when_projected_then_completeness_is_exact() {
    let complete = envelope("complete", NOW);
    let gap = envelope("gap", NOW);
    let filtered = envelope("filtered", NOW);
    let undelivered = envelope("no_delivery", NOW);
    let mut output = chunks(&complete, &[0, 1, 2]);
    output.extend(chunks(&gap, &[0, 2]));
    output.extend(chunks(&filtered, &[0, 2]));
    output.extend(chunks(&undelivered, &[0, 1, 2]));
    let deliveries = vec![
        delivery_row(&complete, "output", (&delivered(0, 0, 3), true), NOW),
        delivery_row(&gap, "output", (&delivered(0, 0, 2), true), NOW),
        delivery_row(&filtered, "output", (&delivered(1, 0, 2), true), NOW),
    ];
    let test_cases = [ProjectionTestCase {
        description: "only gap-free delivered output is complete and gaps are explained",
        batches: vec![output, deliveries],
        sql: "SELECT invocation_id, chunks, missing_sequences, filtered, gap_reason, complete \
              FROM output_invocations ORDER BY invocation_id",
        expected_rows: vec![
            vec![
                text("complete"),
                integer(3),
                integer(0),
                integer(0),
                RecallValue::Null,
                integer(1),
            ],
            vec![
                text("filtered"),
                integer(2),
                integer(1),
                integer(1),
                text("filtered"),
                integer(0),
            ],
            vec![
                text("gap"),
                integer(2),
                integer(1),
                integer(0),
                text("unexplained"),
                integer(0),
            ],
            vec![
                text("no_delivery"),
                integer(3),
                integer(0),
                integer(0),
                RecallValue::Null,
                integer(0),
            ],
        ],
    }];

    for test_case in test_cases {
        assert_eq!(
            projected(&test_case.batches, test_case.sql),
            test_case.expected_rows,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_binary_interleaved_output_when_reconstructed_then_transcripts_are_byte_exact() {
    let env = envelope("binary", NOW);
    let mut reversed: Vec<Vec<_>> = (0_u64..)
        .zip(BINARY_PIECES)
        .map(|(sequence, (stream, data))| vec![chunk(&env, (sequence, stream, data))])
        .collect();
    reversed.reverse();
    let stdout: Vec<u8> = [BINARY_PIECES[0].1, BINARY_PIECES[2].1].concat();
    let stderr: Vec<u8> = [BINARY_PIECES[1].1, BINARY_PIECES[3].1].concat();
    let combined: Vec<u8> = BINARY_PIECES
        .iter()
        .flat_map(|(_, data)| data.iter().copied())
        .collect();
    let test_cases = [
        ProjectionTestCase {
            description: "out-of-order writes reconstruct each stream and the combined order",
            batches: reversed.clone(),
            sql: "SELECT stdout, stderr, combined FROM output_transcripts \
                  WHERE invocation_id = 'binary'",
            expected_rows: vec![vec![
                RecallValue::Blob(stdout),
                RecallValue::Blob(stderr),
                RecallValue::Blob(combined),
            ]],
        },
        ProjectionTestCase {
            description: "the tail lists every chunk",
            batches: reversed,
            sql: "SELECT COUNT(*) FROM output_tail WHERE invocation_id = 'binary'",
            expected_rows: vec![vec![integer(4)]],
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            projected(&test_case.batches, test_case.sql),
            test_case.expected_rows,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_duplicates_and_conflicts_when_written_then_first_row_is_kept_and_conflicts_reported() {
    let env = envelope("dup", NOW);
    let original = chunk(&env, (0, OutputStream::Stdout, b"first"));
    let conflicting = chunk(&env, (0, OutputStream::Stdout, b"second"));
    let writes = vec![
        vec![original.clone()],
        vec![original.clone()],
        vec![conflicting.clone()],
    ];
    // An earlier version stored the full 64-digit digest, of which today's is the prefix.
    let mut legacy = original.clone();
    let full_hash = RecallValue::Text(format!(
        "{}{}",
        original.content_hash().expect("content hash"),
        "0".repeat(32)
    ));
    legacy
        .values
        .iter_mut()
        .filter(|(name, _)| *name == "content_hash")
        .for_each(|(_, value)| *value = full_hash.clone());
    let test_cases = [
        ProjectionTestCase {
            description: "the first row wins",
            batches: writes.clone(),
            sql: "SELECT data FROM recall_output",
            expected_rows: vec![vec![RecallValue::Blob(b"first".to_vec())]],
        },
        ProjectionTestCase {
            description: "only the differing rewrite is a conflict",
            batches: writes,
            sql: "SELECT invocation_id, dataset, record_key FROM conflicts",
            expected_rows: vec![vec![text("dup"), text("recall_output"), text("dup:0")]],
        },
        ProjectionTestCase {
            description: "identical rewrites are not conflicts",
            batches: vec![vec![original.clone()], vec![original.clone()]],
            sql: "SELECT COUNT(*) FROM conflicts",
            expected_rows: vec![vec![integer(0)]],
        },
        ProjectionTestCase {
            description: "a row stored with a full-length digest matches its rewrite, not different content",
            batches: vec![
                vec![legacy.clone()],
                vec![original.clone()],
                vec![conflicting],
            ],
            sql: "SELECT record_key FROM conflicts",
            expected_rows: vec![vec![text("dup:0")]],
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            projected(&test_case.batches, test_case.sql),
            test_case.expected_rows,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_terminals_and_heartbeats_when_projected_then_status_never_claims_false_success() {
    let done = envelope("done", NOW);
    let failed = envelope("failed_late", NOW);
    let running = envelope("running", NOW);
    let abandoned = envelope("abandoned", "2026-10-01T12:00:00.000000Z");
    let unknown = envelope("unknown", NOW);
    let facts = vec![
        event(
            &done,
            ("d0", "invocation_started", 0),
            "",
            r#"{"command":"build"}"#,
        ),
        event(
            &done,
            ("d1", "invocation_completed", 9),
            "",
            r#"{"command":"build","duration_ms":12.5,"exit_code":0}"#,
        ),
        event(
            &failed,
            ("f1", "invocation_completed", 3),
            "",
            r#"{"command":"build","exit_code":0}"#,
        ),
        event(
            &failed,
            ("f2", "invocation_failed", 4),
            "",
            r#"{"command":"build","exit_code":1}"#,
        ),
        event(
            &unknown,
            ("u0", "invocation_started", 0),
            "",
            r#"{"command":"run"}"#,
        ),
        invocation_row(
            &running,
            &InvocationContext {
                command: Some("build".to_owned()),
                target: Some("dev".to_owned()),
                ..InvocationContext::default()
            },
        ),
        heartbeat_row(
            &running,
            (&wall_clock(-1), &wall_clock(90)),
            (None, Some(1)),
            (None, None),
        ),
        heartbeat_row(
            &abandoned,
            ("2026-10-01T12:00:30.000000Z", "2026-10-01T12:02:00.000000Z"),
            (None, None),
            (None, None),
        ),
        heartbeat_row(
            &abandoned,
            ("2026-10-01T12:00:00.000000Z", "2026-10-01T12:01:30.000000Z"),
            (None, None),
            (None, None),
        ),
    ];
    let test_cases = [ProjectionTestCase {
        description: "failures outrank completions and stale heartbeats are abandoned",
        batches: vec![facts],
        sql: "SELECT invocation_id, status, ended_at, duration_ms, exit_code, command, target \
              FROM invocations ORDER BY invocation_id",
        expected_rows: vec![
            vec![
                text("abandoned"),
                text("abandoned"),
                text("2026-10-01T12:00:30.000000Z"),
                RecallValue::Real(30000.0),
                RecallValue::Null,
                RecallValue::Null,
                RecallValue::Null,
            ],
            vec![
                text("done"),
                text("completed"),
                text(NOW),
                RecallValue::Real(12.5),
                integer(0),
                text("build"),
                RecallValue::Null,
            ],
            vec![
                text("failed_late"),
                text("failed"),
                text(NOW),
                RecallValue::Null,
                integer(1),
                RecallValue::Null,
                RecallValue::Null,
            ],
            vec![
                text("running"),
                text("running"),
                RecallValue::Null,
                RecallValue::Null,
                RecallValue::Null,
                text("build"),
                text("dev"),
            ],
            vec![
                text("unknown"),
                text("unknown"),
                RecallValue::Null,
                RecallValue::Null,
                RecallValue::Null,
                text("run"),
                RecallValue::Null,
            ],
        ],
    }];

    for test_case in test_cases {
        assert_eq!(
            projected(&test_case.batches, test_case.sql),
            test_case.expected_rows,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_statement_facts_when_projected_then_failures_and_missing_terminals_stay_visible() {
    let env = envelope("stmts", NOW);
    let digest = sha256_hex(STATEMENT_SQL.as_bytes());
    let payload = format!(
        r#"{{"intent":"execute","statement_kind":"SELECT","purpose":"state_write","sql_digest":"{digest}"}}"#
    );
    let failed_payload = format!(
        r#"{{"intent":"execute","statement_kind":"SELECT","sql_digest":"{digest}","error_type":"DatabaseError","error_code":"42P01","query_id":"q9"}}"#
    );
    let mut facts = vec![
        event(
            &env,
            ("r0", "resource_attempt_started", 0),
            r#","run_id":"run","resource_id":"orders","resource_attempt_id":"a1""#,
            r#"{"attempt_number":1,"resource_kind":"model","resource_name":"orders"}"#,
        ),
        event(
            &env,
            ("s0", "statement_started", 1),
            r#","statement_id":"s1","resource_id":"orders""#,
            &payload,
        ),
        event(
            &env,
            ("s1", "statement_failed", 2),
            r#","statement_id":"s1","resource_id":"orders""#,
            &failed_payload,
        ),
        event(
            &env,
            ("s2", "statement_started", 3),
            r#","statement_id":"s2""#,
            r#"{"intent":"query","statement_kind":"SELECT"}"#,
        ),
    ];
    facts.extend(artifact_rows(
        &env,
        &ArtifactInput {
            digest: format!("sha256:{digest}"),
            kind: "sql".to_owned(),
            media_type: "text/x-sql".to_owned(),
            content: STATEMENT_SQL.as_bytes().to_vec(),
            seen_at: NOW.to_owned(),
            ref_id: "s1".to_owned(),
            role: "executed_sql".to_owned(),
            statement_id: Some("s1".to_owned()),
            resource_id: Some("orders".to_owned()),
        },
    ));
    let test_cases = [
        ProjectionTestCase {
            description: "statements join resources, terminals and their SQL",
            batches: vec![facts.clone()],
            sql: "SELECT statement_id, resource_name, purpose, outcome, error_code, query_id, \
                  sql_available FROM statements ORDER BY statement_id",
            expected_rows: vec![
                vec![
                    text("s1"),
                    text("orders"),
                    text("state_write"),
                    text("failed"),
                    text("42P01"),
                    text("q9"),
                    integer(1),
                ],
                vec![
                    text("s2"),
                    RecallValue::Null,
                    RecallValue::Null,
                    text("no_terminal"),
                    RecallValue::Null,
                    RecallValue::Null,
                    integer(0),
                ],
            ],
        },
        ProjectionTestCase {
            description: "artifacts decode their SQL text",
            batches: vec![facts],
            sql: "SELECT available, text FROM artifacts",
            expected_rows: vec![vec![integer(1), text(STATEMENT_SQL)]],
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            projected(&test_case.batches, test_case.sql),
            test_case.expected_rows,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_audit_completed_facts_when_projected_then_audit_results_carry_outcome_and_target() {
    let env = envelope("audits", NOW);
    let test_cases = [ProjectionTestCase {
        description: "audit outcomes name the audited relation and column",
        batches: vec![vec![event(
            &env,
            ("a0", "audit_completed", 1),
            r#","run_id":"run""#,
            r#"{"audit_name":"revenue_positive","audit_definition_name":"positive","evaluation_mode":"violations","outcome":"warn","severity":"warn","run_scope_phase":"model","attachment_kind":"column","target_schema":"analytics","target_name":"daily_revenue","attached_column_name":"total_revenue_cents","violation_count":3,"sql_digest":"ab"}"#,
        )]],
        sql: "SELECT audit_name, target_relation, \"column\", outcome, violation_count \
              FROM audit_results",
        expected_rows: vec![vec![
            text("revenue_positive"),
            text("analytics.daily_revenue"),
            text("total_revenue_cents"),
            text("warn"),
            integer(3),
        ]],
    }];

    for test_case in test_cases {
        assert_eq!(
            projected(&test_case.batches, test_case.sql),
            test_case.expected_rows,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_the_same_sql_in_two_invocations_when_written_then_artifacts_are_shared_without_conflicts()
{
    let first = envelope("first", NOW);
    let second = envelope("second", &wall_clock(60));
    let seen_twice = || {
        let mut rows = artifact_rows(&first, &artifact("sha256:shared", NOW)).to_vec();
        rows.extend(artifact_rows(
            &second,
            &artifact("sha256:shared", &wall_clock(60)),
        ));
        vec![rows]
    };
    let test_cases = [
        ProjectionTestCase {
            description: "a repeated artifact is a duplicate, not a conflict",
            batches: seen_twice(),
            sql: "SELECT COUNT(*) FROM conflicts",
            expected_rows: vec![vec![integer(0)]],
        },
        ProjectionTestCase {
            description: "one content row serves both invocations' references",
            batches: seen_twice(),
            sql: "SELECT (SELECT COUNT(*) FROM recall_artifacts), \
                  (SELECT COUNT(*) FROM artifacts WHERE available = 1)",
            expected_rows: vec![vec![integer(1), integer(2)]],
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            projected(&test_case.batches, test_case.sql),
            test_case.expected_rows,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_lifecycle_payloads_when_stored_then_promoted_fields_are_kept_once_and_read_back_whole() {
    let written = r#"{"intent":"execute","statement_kind":"SELECT","duration_ms":12.5,"affected_rows":3,"query_id":null,"attempt_number":2}"#;
    let test_cases = [
        PayloadTestCase {
            description: "the table keeps real fields and fields without a column of their own",
            sql: "SELECT payload FROM recall_events",
            expected_payload: r#"{"duration_ms":12.5,"query_id":null,"attempt_number":2}"#,
        },
        PayloadTestCase {
            description: "the events view returns the payload as written, explicit nulls included",
            sql: "SELECT payload FROM events",
            expected_payload: written,
        },
    ];
    let env = envelope("strip", NOW);
    let batches = [vec![event(
        &env,
        ("e0", "statement_completed", 0),
        r#","statement_id":"s1""#,
        written,
    )]];
    for test_case in test_cases {
        let rows = projected(&batches, test_case.sql);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&rows[0][0].key_text()).expect("JSON"),
            serde_json::from_str::<serde_json::Value>(test_case.expected_payload).expect("JSON"),
            "{}",
            test_case.description
        );
    }
}
