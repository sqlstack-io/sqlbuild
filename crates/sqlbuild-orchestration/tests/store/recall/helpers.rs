//! Recall publication fixtures exercise the control store's durable append boundary.

use serde_json::json;
use sqlbuild_orchestration::store::main::migrate_store::migrate_store;
use sqlbuild_orchestration::store::main::publish_recall::publish_recall;
use sqlbuild_orchestration::store::main::read_recall_events::read_recall_events;
use sqlbuild_recall::records::main::artifact_rows::artifact_rows;
use sqlbuild_recall::records::main::event_object_row::event_object_row;
use sqlbuild_recall::records::models::{ArtifactInput, RecallRow, RowEnvelope};

use crate::replica_helpers::second_store;
use crate::store_helpers::{close_store, close_test_store, open_test_store};
use crate::store_test_types::Backend;

pub(crate) fn event(id: &str, occurred_at: &str, duration_ms: f64) -> RecallRow {
    event_object_row(&RowEnvelope { invocation_id: "orders-build".into(),
        invocation_started_at: "2026-03-01T00:30:00+01:00".into(), producer_version: "test".into() },
        json!({ "event_id": id, "event_type": "invocation_completed", "schema_version": 1,
            "occurred_at": occurred_at, "producer": "worker", "invocation_sequence": 1,
            "payload": { "duration_ms": duration_ms, "exit_code": 0, "nested": [null, true, "orders"] } })
            .as_object().expect("event object")).expect("valid lifecycle event")
}

pub(crate) async fn recall_contract(backend: Backend) -> Option<Vec<String>> {
    let fixture = open_test_store(backend).await?;
    let store = &fixture.store;
    migrate_store(store)
        .await
        .expect("migrate control and Recall schema");
    let second = second_store(store).await;
    let first = event("orders-first", "2026-03-01T00:00:00Z", 3.25);
    let first_batch = [first.clone()];
    let (left, right) = tokio::join!(
        publish_recall(store, &first_batch),
        publish_recall(&second, &first_batch)
    );
    let left = left.expect("first writer").remove(0);
    let right = right.expect("second writer").remove(0);
    assert_ne!(left.inserted, right.inserted, "exactly one writer inserts");
    assert_eq!(
        left.storage_id, right.storage_id,
        "duplicates preserve cursor"
    );
    let cursor = left.storage_id.expect("event storage cursor");
    let conflict = publish_recall(store, &[event("orders-first", "2026-03-01T00:00:00Z", 7.5)])
        .await
        .expect("record conflicting retry")
        .remove(0);
    assert!(
        conflict.conflict.is_some(),
        "different content records conflict"
    );
    let later = event("orders-later-arrival", "2026-02-01T00:00:00Z", 2.5);
    let malformed = event("orders-malformed", "2026-04-01T00:00:00Z", 1.0);
    let mut malformed = malformed;
    malformed.values.retain(|(name, _)| *name != "event_id");
    assert!(
        publish_recall(store, &[later.clone(), malformed])
            .await
            .is_err(),
        "whole malformed batch rolls back"
    );
    assert!(
        read_recall_events(store, cursor, 100)
            .await
            .expect("rolled-back page")
            .is_empty()
    );
    publish_recall(store, &[later])
        .await
        .expect("retry rolled-back event");
    let page = read_recall_events(store, cursor, 1)
        .await
        .expect("late arrival page");
    assert_eq!(page.len(), 1);
    assert_eq!(
        page[0]["event_id"], "orders-later-arrival",
        "page follows commit order, not producer time"
    );
    let rows = read_recall_events(store, 0, 100).await.expect("all events");
    assert_eq!(
        rows[0]["duration_ms"],
        json!(3.25),
        "first content wins with fractional precision"
    );
    let payload = rows[0]["payload"].as_str().map_or_else(
        || rows[0]["payload"].clone(),
        |text| serde_json::from_str(text).expect("JSON payload"),
    );
    assert_eq!(payload["nested"], json!([null, true, "orders"]));
    let artifact = ArtifactInput {
        digest: "sha256:orders".into(),
        kind: "sql".into(),
        media_type: "application/octet-stream".into(),
        content: vec![0, 255, 1, 0],
        seen_at: "2026-01-01T00:00:00Z".into(),
        ref_id: "orders".into(),
        role: "input".into(),
        statement_id: None,
        resource_id: None,
    };
    let first_artifact = artifact_rows(
        &RowEnvelope {
            invocation_id: "first-artifact".into(),
            invocation_started_at: artifact.seen_at.clone(),
            producer_version: "test".into(),
        },
        &artifact,
    );
    publish_recall(store, &first_artifact)
        .await
        .expect("first binary artifact");
    let artifact = ArtifactInput {
        seen_at: "2026-04-01T00:00:00Z".into(),
        ..artifact
    };
    let second_artifact = artifact_rows(
        &RowEnvelope {
            invocation_id: "second-artifact".into(),
            invocation_started_at: artifact.seen_at.clone(),
            producer_version: "test".into(),
        },
        &artifact,
    );
    let outcomes = publish_recall(store, &second_artifact)
        .await
        .expect("same artifact across months");
    assert!(
        !outcomes[0].inserted && outcomes[0].conflict.is_none(),
        "artifact identity is global, not partition-local"
    );
    assert!(outcomes[1].inserted, "each invocation keeps its reference");
    let ids = rows
        .iter()
        .map(|row| row["event_id"].as_str().expect("event ID").to_owned())
        .collect();
    close_store(second).await;
    close_test_store(fixture).await;
    Some(ids)
}
