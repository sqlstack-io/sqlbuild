//! Snapshots, reader sessions, build pages and change notices for node state API tests.

use serde_json::{Value, json};
use uuid::Uuid;

use crate::api::tests::changes_helpers::next_notice;
use crate::credentials::tests::role_helpers::{SUPPORT_ROLE, orders_grant};
use crate::platform::main::health::report_location_health::report_location_health;
use crate::platform::models::{LocationHealth, LocationHealthReport};
use crate::runs::tests::helpers::snapshot;
use crate::snapshots::models::{SnapshotInput, SnapshotNode};
use crate::store::models::ControlStore;
use crate::warehouse_metadata::models::MetadataBatch;

/// The orders location's snapshot with the support role and one built model per name.
pub(crate) fn orders_snapshot(location: Uuid, names: &[&str]) -> SnapshotInput {
    let mut input = snapshot(location);
    input.documents.insert(
        "roles".into(),
        json!({SUPPORT_ROLE: {"description": "Answer order questions",
            "grants": [orders_grant(json!(["view"]), &["orders"])]}}),
    );
    for name in names {
        input.nodes.insert(
            format!("model:{name}"),
            SnapshotNode {
                kind: "model".into(),
                definition: json!({"name": name, "destination": {"database": null, "schema": "main", "name": name}}),
                raw_sql: None,
                compiled_sql: None,
                python_source: None,
            },
        );
    }
    input
}

/// A registered location reader session, which metadata pages must name.
pub(crate) async fn reader(store: &ControlStore, location: Uuid) -> Uuid {
    let session_id = Uuid::now_v7();
    report_location_health(
        store,
        location,
        &LocationHealthReport {
            event_id: Uuid::now_v7(),
            session_id,
            target: "development".into(),
            health: LocationHealth::Heartbeat {},
        },
    )
    .await
    .expect("reader");
    session_id
}

/// One recorded build of each named model in this snapshot generation.
pub(crate) fn builds(
    session_id: Uuid,
    (generation, names): (i64, &[&str]),
    at: &str,
) -> MetadataBatch {
    let records: Vec<Value> = names
        .iter()
        .map(|name| {
            json!({"event_id": format!("build-{name}-{at}"), "state_database": null, "state_schema": "build_history",
                "node_type": "model", "node_name": name, "target_database": null, "target_schema": "main",
                "target_name": name, "run_id": "orders-run", "definition_hash": format!("definition-{name}"),
                "version_hash": format!("orders-version-{name}"), "schema_fingerprint": "orders-schema",
                "definition": "SELECT order_id FROM orders", "metadata_json": "{}", "ts": at})
        })
        .collect();
    serde_json::from_value(json!({"kind": "fingerprints", "session_id": session_id,
        "generation": generation, "records": records}))
    .expect("build page")
}

/// The next `count` events' data by topic, because one commit's events arrive in either order.
pub(crate) async fn notices(
    feed: &mut crate::api::tests::test_types::SseConnection,
    count: usize,
) -> Vec<Value> {
    let mut notices = Vec::new();
    for _ in 0..count {
        notices.push(next_notice(feed).await.expect("change event"));
    }
    notices.sort_by_key(|notice| notice["topic"].to_string());
    notices
}
