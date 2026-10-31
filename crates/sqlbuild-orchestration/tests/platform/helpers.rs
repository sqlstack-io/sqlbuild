use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use chrono::{TimeDelta, Utc};
use serde_json::json;
use sqlbuild_orchestration::platform::models::PlatformEvent;
use sqlbuild_orchestration::snapshots::models::{SnapshotInput, SnapshotNode};
use sqlbuild_orchestration::store::errors::ControlError;
use sqlbuild_orchestration::store::main::read_recall_events::read_recall_events;
use sqlbuild_orchestration::store::models::ControlStore;
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::test_types::Observation;

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
