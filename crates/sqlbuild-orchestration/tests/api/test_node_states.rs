//! Held node lists and states: ETag revalidation, pushed state changes, recovery reads and snapshot resets on both control stores.

use std::collections::BTreeSet;

use reqwest::{StatusCode, header};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::api::models::{ApiAuthentication, HumanLogin};
use crate::api::tests::changes_helpers::{next_notice, opened, request_person_feed};
use crate::api::tests::helpers::{start_api_with_authentication, stop_api};
use crate::api::tests::test_types::{NodeStatesTestCase, PushedStatesTestCase};
use crate::credentials::tests::role_helpers::{SUPPORT_ROLE, orders_grant, role_fixture};
use crate::node_states::_helpers::views::{current_view, refresh_view, target_cache};
use crate::node_states::main::start_node_states::start_node_states;
use crate::platform::main::report_location_health::report_location_health;
use crate::platform::models::{LocationHealth, LocationHealthReport};
use crate::runs::main::request_job_run::request_job_run;
use crate::runs::models::JobRunRequest;
use crate::runs::tests::helpers::snapshot;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::snapshots::models::{SnapshotInput, SnapshotNode};
use crate::store::main::migrate_store::migrate_store;
use crate::store::models::ControlStore;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;
use crate::warehouse_metadata::main::ingest::ingest_fingerprints::ingest_fingerprints;
use crate::warehouse_metadata::models::MetadataBatch;

#[test]
fn given_held_node_states_when_people_revalidate_and_builds_land_then_only_changes_are_pushed_and_snapshots_reset()
 {
    let test_cases = [
        NodeStatesTestCase {
            description: "SQLite held node states",
            backend: Backend::Sqlite,
            expected_listed: 5,
        },
        NodeStatesTestCase {
            description: "PostgreSQL held node states",
            backend: Backend::Postgres,
            expected_listed: 5,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let roles = role_fixture(store).await;
            let location = roles.location_id;
            let mut input = orders_snapshot(location, &["orders"]);
            input.generation = 2;
            let published = publish_snapshot(store, &input).await.expect("orders");
            let session_id = reader(store, location).await;
            let api = start_api_with_authentication(
                store,
                ApiAuthentication {
                    login: HumanLogin::Password("https://control.example.com".into()),
                    ..ApiAuthentication::default()
                },
            )
            .await;
            let client = reqwest::Client::new();
            let read = |path: &str, bearer: &str, tag: Option<&str>| {
                let mut request = client
                    .get(format!(
                        "{}/api/control/locations/{location}/targets/development/{path}",
                        api.url
                    ))
                    .header(header::HOST, "control.example.com")
                    .header(header::COOKIE, format!("__Host-sqb-session={bearer}"));
                if let Some(tag) = tag {
                    request = request.header(header::IF_NONE_MATCH, tag);
                }
                request.send()
            };
            let admin = roles.administrator.bearer.clone();
            let etag = |response: &reqwest::Response| {
                response.headers()[header::ETAG]
                    .to_str()
                    .expect("ASCII ETag")
                    .to_owned()
            };

            let listed = read("nodes", &admin, None).await.expect("nodes");
            let first_tag = etag(&listed);
            let nodes: Vec<Value> = listed.json().await.expect("node list");
            let unchanged = read("nodes", &admin, Some(&first_tag))
                .await
                .expect("revalidate");
            let other_tag = read("nodes", &admin, Some("\"elsewhere:1\""))
                .await
                .expect("other tag")
                .status();
            let states: Value = read("node-states", &admin, None)
                .await
                .expect("states")
                .json()
                .await
                .expect("states body");
            assert_eq!(
                (
                    first_tag.clone(),
                    nodes.len(),
                    unchanged.status(),
                    etag(&unchanged),
                    other_tag
                ),
                (
                    format!("\"{}:1\"", published.snapshot_id),
                    test_case.expected_listed,
                    StatusCode::NOT_MODIFIED,
                    first_tag.clone(),
                    StatusCode::OK
                ),
                "{}: the list is tagged with its snapshot and version and revalidates to 304",
                test_case.description
            );
            assert_eq!(
                (
                    &states["snapshot_id"],
                    &states["version"],
                    states["states"]
                        .as_object()
                        .expect("states")
                        .keys()
                        .cloned()
                        .collect::<BTreeSet<_>>(),
                    &states["states"]["model:orders"]["last_build"],
                ),
                (
                    &json!(published.snapshot_id),
                    &json!(1),
                    nodes
                        .iter()
                        .map(|node| node["node_key"].as_str().expect("key").to_owned())
                        .collect(),
                    &Value::Null,
                ),
                "{}: node states cover every listed node at the list's version",
                test_case.description
            );

            let member = roles.member.bearer.clone();
            let denied = [
                read("nodes", &member, None)
                    .await
                    .expect("member nodes")
                    .status(),
                read("nodes", &member, Some(&first_tag))
                    .await
                    .expect("member revalidation")
                    .status(),
                read("node-states", &member, None)
                    .await
                    .expect("member states")
                    .status(),
            ];
            assert_eq!(
                denied,
                [StatusCode::FORBIDDEN; 3],
                "{}: a person without view of every node is refused before any held body or 304",
                test_case.description
            );

            let mut feed =
                opened(request_person_feed(&client, &api, (location, "development", &admin)).await)
                    .await;
            ingest_fingerprints(
                store,
                (location, "development"),
                &builds(session_id, (2, &["orders"]), "2026-01-03T10:00:00Z"),
            )
            .await
            .expect("orders build");
            let pushed = next_notice(&mut feed).await.expect("node status");
            let recovered: Value = read("node-states", &admin, None)
                .await
                .expect("recovery read")
                .json()
                .await
                .expect("recovered states");
            assert_eq!(
                (
                    &pushed["topic"],
                    &pushed["version"],
                    &pushed["previous_version"],
                    pushed["states"]
                        .as_object()
                        .expect("pushed states")
                        .keys()
                        .cloned()
                        .collect::<Vec<_>>(),
                    &pushed["states"]["model:orders"]["last_build"]["version_hash"],
                    &pushed.get("full"),
                ),
                (
                    &json!("node_status:development"),
                    &json!(2),
                    &json!(1),
                    vec!["model:orders".to_owned()],
                    &json!("orders-version-orders"),
                    &None,
                ),
                "{}: a build pushes only the node whose state changed",
                test_case.description
            );
            assert_eq!(
                (&recovered["version"], &recovered["states"]["model:orders"]),
                (&json!(2), &pushed["states"]["model:orders"]),
                "{}: a client with a version gap recovers the pushed state from node-states",
                test_case.description
            );
            let relisted = read("nodes", &admin, Some(&first_tag))
                .await
                .expect("stale revalidation");
            let second_tag = etag(&relisted);
            let relisted: Vec<Value> = relisted.json().await.expect("relisted");
            let orders = relisted
                .iter()
                .find(|node| node["node_key"] == "model:orders")
                .expect("orders listed");
            assert_eq!(
                (second_tag, &orders["last_build"]),
                (
                    format!("\"{}:2\"", published.snapshot_id),
                    &pushed["states"]["model:orders"]["last_build"]
                ),
                "{}: a changed state invalidates the old ETag",
                test_case.description
            );

            let mut jobs = input.clone();
            jobs.generation = 3;
            jobs.documents.insert(
                "jobs".into(),
                json!({"orders": {"description": "Build orders", "selection": ["orders"]}}),
            );
            let republished = publish_snapshot(store, &jobs).await.expect("new snapshot");
            let reset = notices(&mut feed, 2).await;
            let reset_tag = etag(
                &read("nodes", &admin, Some(&first_tag))
                    .await
                    .expect("new snapshot list"),
            );
            assert_eq!(
                (reset, reset_tag),
                (
                    vec![
                        json!({"topic": "node_status:development", "version": 3, "previous_version": 2, "full": true}),
                        json!({"topic": format!("snapshot:{location}")}),
                    ],
                    format!("\"{}:3\"", republished.snapshot_id),
                ),
                "{}: a new snapshot resets the list and tells clients to refetch the states",
                test_case.description
            );
            let request = JobRunRequest {
                run_id: Uuid::now_v7(),
                location_id: location,
                target: "development".into(),
                job_key: "orders".into(),
                reason: "manual verification".into(),
                priority: 0,
                window: None,
                full_refresh: false,
                select: Vec::new(),
                exclude: Vec::new(),
            };
            assert!(
                request_job_run(store, (&roles.context, None), &request)
                    .await
                    .expect("queue job")
            );
            assert_eq!(
                notices(&mut feed, 2).await,
                vec![
                    json!({"topic": "node_status:development", "version": 3, "previous_version": 3, "states": {}}),
                    json!({"topic": "runs"}),
                ],
                "{}: a signal that changes no state keeps the version and pushes no states",
                test_case.description
            );
            stop_api(api).await;
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_builds_of_many_nodes_when_states_refresh_then_large_changes_ask_clients_to_refetch() {
    let test_cases = [
        PushedStatesTestCase {
            description: "a few changed nodes are pushed",
            built: 3,
            expected_pushed: Some(3),
        },
        PushedStatesTestCase {
            description: "more than 2,000 changed nodes are announced in full",
            built: 2_001,
            expected_pushed: None,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(Backend::Sqlite).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let location = role_fixture(store).await.location_id;
            let names: Vec<String> = (0..test_case.built)
                .map(|index| format!("orders_{index:04}"))
                .collect();
            let names: Vec<&str> = names.iter().map(String::as_str).collect();
            let mut input = orders_snapshot(location, &names);
            input.generation = 2;
            publish_snapshot(store, &input)
                .await
                .expect("wide snapshot");
            let session_id = reader(store, location).await;
            let (cache, _) = start_node_states(store, &tokio::sync::broadcast::channel(1).0);
            let mut updates = cache.updates.subscribe();
            let held = current_view(&cache, (location, "development"), None)
                .await
                .expect("first view");
            for chunk in names.chunks(100) {
                ingest_fingerprints(
                    store,
                    (location, "development"),
                    &builds(session_id, (2, chunk), "2026-01-03T10:00:00Z"),
                )
                .await
                .expect("builds");
            }
            let entry = target_cache(&cache, (location, "development"));
            let view = refresh_view(&cache, (location, "development"), (&entry, true))
                .await
                .expect("refresh");
            let update: Value =
                serde_json::from_str(&updates.recv().await.expect("update").data).expect("JSON");
            assert_eq!(
                (
                    held.version,
                    view.version,
                    &update["previous_version"],
                    update["states"].as_object().map(serde_json::Map::len),
                    update["full"].as_bool(),
                ),
                (
                    1,
                    2,
                    &json!(1),
                    test_case.expected_pushed,
                    test_case.expected_pushed.is_none().then_some(true),
                ),
                "{}",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}

/// The orders location's snapshot with the support role and one built model per name.
fn orders_snapshot(location: Uuid, names: &[&str]) -> SnapshotInput {
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
async fn reader(store: &ControlStore, location: Uuid) -> Uuid {
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
fn builds(session_id: Uuid, (generation, names): (i64, &[&str]), at: &str) -> MetadataBatch {
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
async fn notices(
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
