use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;

use crate::platform::models::{PlatformEvent, PlatformEventEnvelope};
use crate::runs::main::claim_next_run::claim_next_run;
use crate::runs::main::publish_run_recall::publish_run_recall;
use crate::runs::models::{RecallBatch, RecallRecord};
use crate::snapshots::models::{SnapshotInput, SnapshotNode};
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;
use crate::store::tests::helpers::read_recall_events;
use chrono::{TimeDelta, Utc};
use serde_json::json;
use sqlbuild_recall::records::models::RecallValue;
use sqlbuild_recall::schema::models::Table;
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::platform::tests::test_types::{Observation, WebhookReceiver};
use crate::runs::tests::helpers::{queue, request};
use crate::scheduler::tests::contracts::helpers::acquire_lease;

pub(crate) fn snapshot(location_id: Uuid) -> SnapshotInput {
    let future = Utc::now() + TimeDelta::days(10);
    let mut triggers = BTreeMap::new();
    for index in 0..101 {
        triggers.insert(
            format!("daily_orders_{index}"),
            json!({
                "description": "Refresh orders", "job_key": "orders",
                "starts_at": "2020-01-01T00:00:00Z",
                "schedule": {"kind": "calendar", "times": ["2020-01-01T00:00:00Z", future],
                    "refresh_at": "2020-01-01T00:00:00Z"}
            }),
        );
    }
    for (name, status) in [("expired_orders", "running"), ("paused_orders", "paused")] {
        triggers.insert(name.into(), json!({
            "description": "Refresh orders", "job_key": "orders", "default_status": status,
            "starts_at": "2020-01-01T00:00:00Z",
            "schedule": {"kind": "calendar", "times": ["2020-01-01T00:00:00Z"], "refresh_at": "2020-01-01T00:00:00Z"}
        }));
    }
    triggers.insert(
        "future_orders".into(),
        json!({
            "description": "Future orders refresh", "job_key": "orders",
            "schedule": {"kind": "calendar", "times": [future], "refresh_at": future}
        }),
    );
    SnapshotInput {
        location_id,
        target: "development".into(),
        generation: 1,
        git_revision: None,
        git_dirty: false,
        git_author: None,
        image_id: None,
        environment_hash: "orders-watchdog-environment".into(),
        sqb_version: env!("CARGO_PKG_VERSION").into(),
        nodes: BTreeMap::from([(
            "orders".into(),
            SnapshotNode {
                kind: "model".into(),
                definition: json!({"materialization": "table"}),
                raw_sql: Some("select 1 as order_id".into()),
                compiled_sql: Some("select 1 as order_id".into()),
                python_source: None,
            },
        )]),
        documents: BTreeMap::from([
            (
                "jobs".into(),
                json!({"orders": {"description": "Build orders", "selection": ["orders"]}}),
            ),
            ("triggers".into(), json!(triggers)),
        ]),
    }
}

pub(crate) async fn wait_for_scans(
    store: &ControlStore,
    replicas: &BTreeSet<Uuid>,
    tasks: &[JoinHandle<Result<(), ControlError>>],
) -> Vec<Observation> {
    tokio::time::timeout(Duration::from_secs(10), async {
        let mut cursor = 0;
        let mut observations = Vec::new();
        let mut scanned = BTreeSet::new();
        for _ in 0..1000 {
            let Some(_) = replicas.difference(&scanned).next() else {
                return observations;
            };
            assert!(
                tasks.iter().all(|task| !task.is_finished()),
                "watchdog failed before scanning"
            );
            let batch: Vec<Observation> = read_recall_events(store, cursor, 1000)
                .await
                .expect("read health facts")
                .into_iter()
                .map(|row| serde_json::from_value(json!(row)).expect("typed platform event"))
                .collect();
            cursor = batch.last().map_or(cursor, |row| row.storage_id);
            scanned.extend(batch.iter().filter_map(|row| {
                let PlatformEvent::Heartbeat { replica_id } = row.payload.platform else {
                    return None;
                };
                Some(replica_id)
            }));
            observations.extend(batch);
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("watchdog exhausted scan completion attempts");
    })
    .await
    .expect("both replicas finish a paged scan without a scheduler")
}

pub(crate) async fn receive_webhook(
    State(receiver): State<WebhookReceiver>,
    headers: HeaderMap,
    Json(event): Json<PlatformEventEnvelope>,
) -> impl IntoResponse {
    assert_eq!(
        headers.get("Idempotency-Key").expect("deduplication key"),
        event.event_id.to_string().as_str()
    );
    let mut state = receiver.state.lock().expect("receiver state");
    state.requests.push(event);
    receiver.delivered.send_replace(state.requests.len());
    (
        StatusCode::from_u16(state.status).expect("configured response status"),
        [(axum::http::header::LOCATION, "/events")],
    )
}

pub(crate) async fn wait_for_webhook(
    receiver: &WebhookReceiver,
    dispatcher: &JoinHandle<Result<(), ControlError>>,
    count: usize,
) {
    let mut delivered = receiver.delivered.subscribe();
    tokio::time::timeout(
        Duration::from_secs(10),
        delivered.wait_for(|received| *received >= count),
    )
    .await
    .expect("dispatcher drains more than one bounded page")
    .expect("webhook receiver remains available");
    assert!(
        !dispatcher.is_finished(),
        "delivery loop stopped before draining"
    );
}

pub(crate) async fn publish_platform_impostor(store: &ControlStore, identity: (Uuid, Uuid)) {
    let mut requested = request(identity, 42, 0, &["orders"]);
    requested.image_id = None;
    queue(store, &requested).await;
    let lease = acquire_lease(store, Uuid::now_v7())
        .await
        .expect("elect")
        .expect("leader");
    let claim = claim_next_run(store, &lease, &BTreeMap::new(), None)
        .await
        .expect("claim")
        .expect("work");
    let batch = RecallBatch {
        invocation_id: "platform/impostor".into(),
        records: vec![RecallRecord {
            table: Table::Events,
            values: BTreeMap::from([
                (
                    "event_id".into(),
                    RecallValue::Text(Uuid::now_v7().to_string()),
                ),
                (
                    "event_type".into(),
                    RecallValue::Text("platform_heartbeat".into()),
                ),
                ("event_schema_version".into(), RecallValue::Integer(1)),
                ("event_kind".into(), RecallValue::Text("runtime".into())),
                ("severity".into(), RecallValue::Text("info".into())),
                (
                    "occurred_at".into(),
                    RecallValue::Text("2026-01-01T00:00:00Z".into()),
                ),
                ("producer".into(), RecallValue::Text("platform".into())),
                ("invocation_sequence".into(), RecallValue::Integer(0)),
                (
                    "payload".into(),
                    RecallValue::Text(
                        json!({"platform": {"kind": "heartbeat", "replica_id": Uuid::now_v7()}})
                            .to_string(),
                    ),
                ),
            ]),
        }],
    };
    let result = publish_run_recall(store, (claim.request.run_id, claim.claim_id), &batch)
        .await
        .expect("publish untrusted producer");
    assert_eq!(result.delivered, [(Table::Events, 1)]);
    assert!(result.conflicts.is_empty());
}
