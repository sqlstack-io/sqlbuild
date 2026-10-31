//! Exercise native writer delivery and authenticated retries against both physical stores.

use std::time::Duration;

use serde_json::json;
use sqlbuild_orchestration::runs::models::{RecallBatch, RecallRecord};
use sqlbuild_orchestration::store::main::read_recall_view::read_recall_view;
use sqlbuild_orchestration::store::models::ControlStore;
use sqlbuild_orchestration::worker::errors::WorkerError;
use sqlbuild_orchestration::worker::models::WorkerClient;
use sqlbuild_recall::records::main::artifact_rows::artifact_rows;
use sqlbuild_recall::records::main::event_object_row::event_object_row;
use sqlbuild_recall::records::main::sha256_hex::sha256_hex;
use sqlbuild_recall::records::models::{
    ArtifactInput, InvocationContext, OutputStream, RecallRow, RecallValue, RowEnvelope,
};
use sqlbuild_recall::schema::main::table_spec::table_spec;
use sqlbuild_recall::schema::models::Table;
use sqlbuild_recall::writer::main::start_recall_session::start_recall_session;
use sqlbuild_recall::writer::models::SessionConfig;
use uuid::Uuid;

use crate::output_helpers::stored_bytes;

fn record(row: RecallRow) -> RecallRecord {
    RecallRecord {
        table: row.table,
        values: table_spec(row.table)
            .columns
            .iter()
            .filter_map(|column| {
                row.value(column.name)
                    .map(|value| (column.name.to_owned(), value.clone()))
            })
            .collect(),
    }
}

pub(crate) async fn recall_contract(
    store: &ControlStore,
    worker: &WorkerClient,
    claim_id: Uuid,
) -> RecallBatch {
    let envelope = RowEnvelope {
        invocation_id: "execution".into(),
        invocation_started_at: "2026-01-02T03:04:05Z".into(),
        producer_version: "untrusted-version".into(),
    };
    let invocation = format!("worker/{claim_id}/execution");
    let event = json!({
        "event_id": "same-event-id", "occurred_at": "2026-01-02T03:04:05Z",
        "event_type": "statement_completed", "schema_version": 1, "producer": "executor",
        "statement_id": "shared-producer-id",
        "payload": {"duration_ms": 12.75, "row_count": 9, "outcome": "success"}
    });
    let mut batch = RecallBatch {
        invocation_id: envelope.invocation_id.clone(),
        records: vec![record(
            event_object_row(&envelope, event.as_object().expect("event object"))
                .expect("encode event"),
        )],
    };
    for _ in 0..2 {
        let outcome = worker
            .publish_recall(&batch)
            .await
            .expect("retry event batch");
        assert_eq!(outcome.delivered, [(Table::Events, 1)]);
        assert!(outcome.conflicts.is_empty());
    }
    let events = read_recall_view(store, ("events", &invocation), 10)
        .await
        .expect("read stored events");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["duration_ms"], json!(12.75));
    assert_eq!(events[0]["row_count"], json!(9));
    assert_ne!(events[0]["event_id"], json!("same-event-id"));
    assert_ne!(events[0]["producer_version"], json!("untrusted-version"));
    batch.records[0]
        .values
        .insert("duration_ms".into(), RecallValue::Real(13.25));
    assert_eq!(
        worker
            .publish_recall(&batch)
            .await
            .expect("publish conflicting event")
            .conflicts
            .len(),
        1
    );
    assert_eq!(
        read_recall_view(store, ("events", &invocation), 10)
            .await
            .expect("read original event")[0]["duration_ms"],
        json!(12.75)
    );
    assert_eq!(
        read_recall_view(store, ("conflicts", &invocation), 10)
            .await
            .expect("read conflict evidence")
            .len(),
        1
    );

    let mut separate = batch.clone();
    separate.invocation_id = "another-invocation".into();
    assert_eq!(
        worker
            .publish_recall(&separate)
            .await
            .expect("publish another invocation")
            .delivered,
        [(Table::Events, 1)]
    );
    assert_eq!(
        read_recall_view(store, ("statements", &invocation), 10)
            .await
            .expect("read first invocation statement")[0]["duration_ms"],
        json!(12.75)
    );
    assert_eq!(
        read_recall_view(
            store,
            (
                "statements",
                &format!("worker/{claim_id}/another-invocation")
            ),
            10
        )
        .await
        .expect("read second invocation statement")[0]["duration_ms"],
        json!(13.25)
    );
    let artifact = ArtifactInput {
        digest: format!("sha256:{}", sha256_hex(b"orders\0\xff")),
        kind: "evidence".into(),
        media_type: "application/octet-stream".into(),
        content: b"orders\0\xff".to_vec(),
        seen_at: "2026-01-02T03:04:05Z".into(),
        ref_id: "first".into(),
        role: "evidence".into(),
        statement_id: None,
        resource_id: None,
    };
    let [content, reference] = artifact_rows(&envelope, &artifact).map(record);
    let artifacts = RecallBatch {
        invocation_id: "execution".into(),
        records: vec![content.clone(), reference.clone()],
    };
    assert_eq!(
        worker
            .publish_recall(&artifacts)
            .await
            .expect("publish artifact with proof")
            .delivered
            .len(),
        2
    );
    let mut later_ref = reference.clone();
    later_ref
        .values
        .insert("ref_id".into(), RecallValue::Text("second".into()));
    assert_eq!(
        worker
            .publish_recall(&RecallBatch {
                invocation_id: "execution".into(),
                records: vec![later_ref]
            })
            .await
            .expect("publish later artifact reference")
            .delivered,
        [(Table::ArtifactRefs, 1)]
    );
    assert_eq!(
        read_recall_view(store, ("artifacts", &invocation), 10)
            .await
            .expect("read both artifact references")
            .len(),
        2
    );

    let mut forged = content;
    forged
        .values
        .insert("content".into(), RecallValue::Blob(b"different".to_vec()));
    let mut event_next = batch.records[0].clone();
    event_next.values.insert(
        "event_id".into(),
        RecallValue::Text("must-roll-back".into()),
    );
    let mut forged_envelope = event_next.clone();
    forged_envelope.values.insert(
        "invocation_id".into(),
        RecallValue::Text("scheduler/other".into()),
    );
    let mut wrong_type = event_next.clone();
    wrong_type
        .values
        .insert("row_count".into(), RecallValue::Text("nine".into()));
    let mut missing = event_next.clone();
    missing.values.remove("event_id");
    let mut bad_json = event_next.clone();
    bad_json
        .values
        .insert("payload".into(), RecallValue::Text("not-json".into()));
    let mut bad_timestamp = event_next.clone();
    bad_timestamp
        .values
        .insert("occurred_at".into(), RecallValue::Text("not-a-time".into()));
    for records in [
        vec![event_next.clone(), forged],
        vec![event_next.clone(), forged_envelope],
        vec![event_next.clone(), wrong_type],
        vec![event_next.clone(), missing],
        vec![event_next.clone(), bad_json],
        vec![event_next.clone(), bad_timestamp],
        vec![
            event_next.clone(),
            RecallRecord {
                table: Table::Meta,
                values: Default::default(),
            },
        ],
        vec![
            event_next.clone(),
            RecallRecord {
                table: Table::Conflicts,
                values: Default::default(),
            },
        ],
        vec![event_next; 501],
    ] {
        assert!(matches!(
            worker
                .publish_recall(&RecallBatch {
                    invocation_id: "execution".into(),
                    records
                })
                .await,
            Err(WorkerError::Rejected(reqwest::StatusCode::BAD_REQUEST))
        ));
    }
    // A later authorization failure rolls back the preceding valid event, not just validation errors.
    assert!(matches!(
        worker
            .publish_recall(&RecallBatch {
                invocation_id: "foreign".into(),
                records: vec![batch.records[0].clone(), reference]
            })
            .await,
        Err(WorkerError::Rejected(reqwest::StatusCode::BAD_REQUEST))
    ));
    assert!(
        read_recall_view(store, ("events", &format!("worker/{claim_id}/foreign")), 10)
            .await
            .expect("read rolled-back invocation")
            .is_empty()
    );
    assert_eq!(
        read_recall_view(store, ("events", &invocation), 10)
            .await
            .expect("read unchanged invocation")
            .len(),
        1
    );

    native_session(store, worker, claim_id).await;
    large_batches(store, worker, claim_id).await;
    batch
}

async fn native_session(store: &ControlStore, worker: &WorkerClient, claim_id: Uuid) {
    let hosted = worker.recall_store();
    let summary = tokio::task::spawn_blocking(move || {
        let session = start_recall_session(SessionConfig {
            store: hosted,
            envelope: RowEnvelope { invocation_id: "native-writer".into(), invocation_started_at: "2026-01-02T03:04:05Z".into(), producer_version: "test-version".into() },
            context: InvocationContext { command: Some("build".into()), git_dirty: Some(true), ..Default::default() },
            retention_days: 30, shutdown_timeout: Duration::from_secs(10), diagnostics_min_severity: "debug".into(),
            max_output_bytes: None, heartbeat_interval: Duration::from_secs(60), setup_budget: Duration::from_secs(2),
            maintenance: false, defer_context: false, maintenance_owner: "test-worker".into(), audit_evidence: Default::default(),
        });
        session.append_output(OutputStream::Stdout, b"customers\0\xff\n");
        session.append_output(OutputStream::Stderr, b"warning\xc3\x28\n");
        session.publish_sql("select 7 as order_id", Some("statement-1"), Some("orders"));
        session.publish_diagnostic(&json!({"occurred_at": "2026-01-02T03:04:05Z", "severity": "warning", "logger": "sqlbuild", "source": "test", "message": "orders warning", "fields": {"count": 3}}).to_string()).expect("queue diagnostic");
        session.publish_event(&json!({"event_id": "native-event", "occurred_at": "2026-01-02T03:04:05Z", "event_type": "statement_completed", "producer": "executor", "schema_version": 1, "payload": {"duration_ms": 2.5}}).to_string()).expect("queue event");
        session.close()
    }).await.expect("native writer thread");
    assert!(summary.flush_complete, "{summary:?}");
    assert_eq!(summary.total().accepted, summary.total().delivered);
    let invocation = format!("worker/{claim_id}/native-writer");
    let output = stored_bytes(store, &invocation).await;
    assert_eq!(output.len(), 2);
    assert!(
        output
            .iter()
            .any(|(_, bytes)| bytes == b"customers\0\xff\n")
    );
    assert!(
        output
            .iter()
            .any(|(_, bytes)| bytes == b"warning\xc3\x28\n")
    );
    assert_eq!(
        read_recall_view(store, ("diagnostics", &invocation), 10)
            .await
            .expect("read diagnostic")[0]["message"],
        json!("orders warning")
    );
    assert_eq!(
        read_recall_view(store, ("events", &invocation), 10)
            .await
            .expect("read native events")
            .len(),
        1
    );
    assert_eq!(
        read_recall_view(store, ("artifacts", &invocation), 10)
            .await
            .expect("read captured SQL")[0]["text"],
        json!("select 7 as order_id")
    );
    let delivered = read_recall_view(store, ("output_invocations", &invocation), 10)
        .await
        .expect("read delivery accounting");
    assert_eq!(delivered.len(), 1);
    assert!(
        delivered[0]["flush_complete"] == json!(1) || delivered[0]["flush_complete"] == json!(true)
    );
}

async fn large_batches(store: &ControlStore, worker: &WorkerClient, claim_id: Uuid) {
    use sqlbuild_recall::writer::models::SessionStore;
    use std::time::Instant;

    let SessionStore::Hosted(opener) = worker.recall_store() else {
        panic!("hosted store")
    };
    tokio::task::spawn_blocking(move || {
        let envelope = RowEnvelope {
            invocation_id: "backlog".into(),
            invocation_started_at: "2026-01-02T03:04:05Z".into(),
            producer_version: "test".into(),
        };
        let store = opener
            .open(&envelope, Instant::now() + Duration::from_secs(2))
            .expect("open native HTTP store");
        // A replacement can expand one pending lifecycle event beyond the queue's batch count.
        let events = (0..501)
            .map(|index| {
                event_object_row(&envelope, json!({
            "event_id": format!("backlog-{index}"), "occurred_at": "2026-01-02T03:04:05Z",
            "event_type": "statement_completed", "producer": "executor", "schema_version": 1,
            "payload": {"row_count": index}
        }).as_object().expect("event object")).expect("encode backlog event")
            })
            .collect::<Vec<_>>();
        let expanded = store.insert(&events, Instant::now() + Duration::from_secs(30));
        // Six maximum-sized native artifacts fit the 32 MiB queue, but their JSON blobs exceed 64 MiB.
        let artifacts = (0..6)
            .flat_map(|index| {
                let content = vec![b'a' + index; 4 * 1024 * 1024];
                artifact_rows(
                    &envelope,
                    &ArtifactInput {
                        digest: format!("sha256:{}", sha256_hex(&content)),
                        content,
                        kind: "evidence".into(),
                        media_type: "application/octet-stream".into(),
                        seen_at: "2026-01-02T03:04:05Z".into(),
                        ref_id: format!("backlog-{index}"),
                        role: "evidence".into(),
                        statement_id: None,
                        resource_id: None,
                    },
                )
            })
            .collect::<Vec<_>>();
        let bytes = store.insert(&artifacts, Instant::now() + Duration::from_secs(30));
        assert!(
            expanded.is_ok() && bytes.is_ok(),
            "materialized rows: {expanded:?}; wire bytes: {bytes:?}"
        );
        assert_eq!(
            expanded.expect("publish expanded event batch").delivered,
            [(Table::Events, 501)]
        );
        assert_eq!(
            bytes.expect("publish large artifact batch").delivered,
            [(Table::Artifacts, 6), (Table::ArtifactRefs, 6)]
        );
    })
    .await
    .expect("join backlog writer");
    let invocation = format!("worker/{claim_id}/backlog");
    assert_eq!(
        read_recall_view(store, ("events", &invocation), 1000)
            .await
            .expect("read full event backlog")
            .len(),
        501
    );
    assert_eq!(
        read_recall_view(store, ("artifacts", &invocation), 10)
            .await
            .expect("read large artifacts")
            .len(),
        6
    );
}
