//! Durable Recall views preserve the published execution records.

use serde_json::json;
use sqlbuild_orchestration::store::main::migrate_store::migrate_store;
use sqlbuild_orchestration::store::main::publish_recall::publish_recall;
use sqlbuild_orchestration::store::main::read_recall_view::read_recall_view;
use sqlbuild_recall::records::main::artifact_rows::artifact_rows;
use sqlbuild_recall::records::main::delivery_row::delivery_row;
use sqlbuild_recall::records::main::event_object_row::event_object_row;
use sqlbuild_recall::records::main::heartbeat_row::heartbeat_row;
use sqlbuild_recall::records::main::invocation_row::invocation_row;
use sqlbuild_recall::records::main::output_row::output_row;
use sqlbuild_recall::records::models::{
    ArtifactInput, DeliveryCounts, InvocationContext, OutputChunk, OutputStream, RowEnvelope,
};

use crate::store_helpers::{close_test_store, open_test_store, run};
use crate::store_test_types::Backend;
use crate::test_types::RecallViewTestCase;

#[test]
fn given_native_context_output_and_audits_when_reading_views_then_both_databases_preserve_their_meaning()
 {
    let test_cases = [
        RecallViewTestCase {
            description: "SQLite supported views",
            backend: Backend::Sqlite,
            expected_complete: 1,
        },
        RecallViewTestCase {
            description: "PostgreSQL supported views",
            backend: Backend::Postgres,
            expected_complete: 1,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate view store");
            let envelope = RowEnvelope {
                invocation_id: "orders-output".into(),
                invocation_started_at: "2026-01-02T00:00:00Z".into(),
                producer_version: "test".into(),
            };
            let context = InvocationContext {
                command: Some("build".into()),
                selection: vec!["orders".into()],
                external_context: Some(
                    json!({"integration": {"name": "orders", "retry_number": 2}}).to_string(),
                ),
                ..InvocationContext::default()
            };
            let mut rows = vec![
                invocation_row(&envelope, &context),
                heartbeat_row(
                    &envelope,
                    ("2026-01-02T00:00:01Z", "2026-01-02T00:00:31Z"),
                    (None, None),
                    (Some("build"), None),
                ),
            ];
            for (sequence, (stream, bytes)) in [
                (OutputStream::Stdout, b"orders\x00\xff".as_slice()),
                (OutputStream::Stderr, b"warning\n".as_slice()),
                (OutputStream::Stdout, b"\xc3\x28 tail".as_slice()),
            ]
            .into_iter()
            .enumerate()
            {
                rows.push(output_row(
                    &envelope,
                    &OutputChunk {
                        sequence: sequence as u64,
                        stream,
                        occurred_at: "2026-01-02T00:00:02Z".into(),
                        data: bytes.to_vec(),
                        byte_offset: 0,
                        dropped_before: 0,
                        omitted: None,
                    },
                ));
            }
            rows.push(delivery_row(
                &envelope,
                "output",
                (
                    &DeliveryCounts {
                        accepted: 3,
                        delivered: 3,
                        ..DeliveryCounts::default()
                    },
                    true,
                ),
                "2026-01-02T00:00:03Z",
            ));
            for (event_id, event_type, payload) in [
                (
                    "orders-audit",
                    "audit_completed",
                    json!({"audit_name":"unique_orders", "violation_count": 7, "measured_value": 3.5, "target_name":"orders", "evidence_truncated": true}),
                ),
                (
                    "orders-complete",
                    "invocation_completed",
                    json!({"duration_ms": 12.75, "exit_code": 0}),
                ),
            ] {
                rows.push(event_object_row(&envelope, json!({"event_id": event_id, "event_type": event_type, "schema_version": 1, "occurred_at": "2026-01-02T00:00:03Z", "producer": "worker", "invocation_sequence": 1, "payload": payload}).as_object().expect("event object")).expect("lifecycle fact"));
            }
            rows.extend(artifact_rows(
                &envelope,
                &ArtifactInput {
                    digest: "sha256:orders-display".into(),
                    kind: "sql".into(),
                    media_type: "text/plain".into(),
                    content: b"orders\x00\xff".to_vec(),
                    seen_at: envelope.invocation_started_at.clone(),
                    ref_id: "orders-statement".into(),
                    role: "input".into(),
                    statement_id: None,
                    resource_id: None,
                },
            ));
            publish_recall(store, &rows)
                .await
                .expect("write view inputs");
            let output =
                read_recall_view(store, ("output_invocations", &envelope.invocation_id), 100)
                    .await
                    .expect("output accounting");
            assert_eq!(output[0]["complete"], json!(test_case.expected_complete));
            assert_eq!(output[0]["chunks"], json!(3));
            assert_eq!(output[0]["missing_sequences"], json!(0));
            let transcript =
                read_recall_view(store, ("output_transcripts", &envelope.invocation_id), 100)
                    .await
                    .expect("binary transcript");
            assert_eq!(
                transcript[0]["stdout"],
                json!([b"orders\x00\xff".as_slice(), b"\xc3\x28 tail".as_slice()].concat())
            );
            assert_eq!(
                transcript[0]["combined"],
                json!(
                    [
                        b"orders\x00\xff".as_slice(),
                        b"warning\n".as_slice(),
                        b"\xc3\x28 tail".as_slice()
                    ]
                    .concat()
                )
            );
            let invocation = read_recall_view(store, ("invocations", &envelope.invocation_id), 100)
                .await
                .expect("invocation projection");
            assert_eq!(invocation[0]["status"], "completed");
            assert_eq!(invocation[0]["duration_ms"], json!(12.75));
            assert_eq!(invocation[0]["integration_retry"], json!(2));
            let audit = read_recall_view(store, ("audit_results", &envelope.invocation_id), 100)
                .await
                .expect("audit projection");
            assert_eq!(audit[0]["violation_count"], json!(7));
            assert_eq!(audit[0]["measured_value"], json!(3.5));
            assert_eq!(audit[0]["evidence_truncated"], json!(1));
            let tail = read_recall_view(store, ("output_tail", &envelope.invocation_id), 100)
                .await
                .expect("lossy output display");
            assert_eq!(
                tail.iter()
                    .find(|row| row["sequence"] == 0)
                    .expect("first chunk")["text"],
                json!("orders\u{0}\u{fffd}"),
                "{}",
                test_case.description
            );
            assert_eq!(
                tail.iter()
                    .find(|row| row["sequence"] == 2)
                    .expect("last chunk")["text"],
                json!("\u{fffd}( tail")
            );
            let artifacts = read_recall_view(store, ("artifacts", &envelope.invocation_id), 100)
                .await
                .expect("lossy artifact display");
            assert_eq!(artifacts[0]["text"], json!("orders\u{0}\u{fffd}"));
            assert_eq!(artifacts[0]["content"], json!(b"orders\x00\xff".to_vec()));
            assert_eq!(artifacts[0]["available"], json!(1));
            close_test_store(fixture).await;
        });
    }
}
