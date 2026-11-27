//! Held node lists and states: ETag revalidation, pushed state changes, recovery reads and snapshot resets on both control stores.

use std::collections::BTreeSet;
use std::time::Duration;

use reqwest::{StatusCode, header};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::api::models::{ApiAuthentication, HumanLogin};
use crate::api::tests::changes_helpers::{next_notice, next_topic, opened, request_person_feed};
use crate::api::tests::helpers::{start_api_with_authentication, stop_api};
use crate::api::tests::node_states_helpers::{builds, notices, orders_snapshot, reader};
use crate::api::tests::test_types::{
    ChangedBuildTestCase, HeldTargetTestCase, NodeStatesTestCase, PushedStatesTestCase,
};
use crate::credentials::tests::role_helpers::role_fixture;
use crate::node_states::_helpers::views::{current_view, refresh_view, target_cache, watch_target};
use crate::node_states::main::report_changed_build::report_changed_build;
use crate::node_states::main::start_node_states::start_node_states;
use crate::node_states::models::ChangedBuildReport;
use crate::platform::main::health::location_health::location_health;
use crate::platform::main::health::report_location_health::report_location_health;
use crate::platform::models::{LocationHealth, LocationHealthReport, LocationState};
use crate::runs::main::requests::request_job_run::request_job_run;
use crate::runs::models::JobRunRequest;
use crate::snapshots::main::publication::publish_snapshot::publish_snapshot;
use crate::store::errors::ControlError;
use crate::store::main::migrate_store::migrate_store;
use crate::store::models::{ChangeMessage, ChangeTopic};
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;
use crate::tls::main::client_builder::client_builder;
use crate::warehouse_metadata::main::ingest::ingest_fingerprints::ingest_fingerprints;
use sqlbuild_recall::records::main::sha256_hex::sha256_hex;

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
            let client = client_builder().build().expect("HTTP client");
            let read = |path: &str, bearer: &str, tag: Option<&str>| {
                let request = client
                    .get(format!(
                        "{}/api/control/locations/{location}/targets/development/{path}",
                        api.url
                    ))
                    .header(header::HOST, "control.example.com")
                    .header(header::COOKIE, format!("__Host-sqb-session={bearer}"));
                tag.into_iter()
                    .fold(request, |request, tag| {
                        request.header(header::IF_NONE_MATCH, tag)
                    })
                    .send()
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
                    format!("\"{}\"", published.snapshot_id),
                    test_case.expected_listed,
                    StatusCode::NOT_MODIFIED,
                    first_tag.clone(),
                    StatusCode::OK
                ),
                "{}: the list is tagged with its snapshot and revalidates to 304",
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
                .expect("revalidation after a build");
            assert_eq!(
                (relisted.status(), etag(&relisted)),
                (StatusCode::NOT_MODIFIED, first_tag.clone()),
                "{}: a changed state keeps the list's ETag, since node-states serves states",
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
                    format!("\"{}\"", republished.snapshot_id),
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
fn given_changed_build_reports_when_people_read_node_states_then_only_the_current_generations_plan_is_served()
 {
    let test_cases = [
        ChangedBuildTestCase {
            description: "SQLite changed build",
            backend: Backend::Sqlite,
            expected_before_plan: Value::Null,
        },
        ChangedBuildTestCase {
            description: "PostgreSQL changed build",
            backend: Backend::Postgres,
            expected_before_plan: Value::Null,
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
            let mut input = orders_snapshot(location, &["orders", "customers"]);
            input.generation = 2;
            publish_snapshot(store, &input).await.expect("orders");
            let session_id = reader(store, location).await;
            let api = start_api_with_authentication(
                store,
                ApiAuthentication {
                    login: HumanLogin::Password("https://control.example.com".into()),
                    ..ApiAuthentication::default()
                },
            )
            .await;
            let client = client_builder().build().expect("HTTP client");
            let admin = roles.administrator.bearer.clone();
            let changed_build = || async {
                let states: Value = client
                    .get(format!(
                        "{}/api/control/locations/{location}/targets/development/node-states",
                        api.url
                    ))
                    .header(header::HOST, "control.example.com")
                    .header(header::COOKIE, format!("__Host-sqb-session={admin}"))
                    .send()
                    .await
                    .expect("states")
                    .json()
                    .await
                    .expect("states body");
                let mut build = states["changed_build"].clone();
                build.as_object_mut().into_iter().for_each(|build| {
                    assert!(build.remove("reported_at").is_some_and(|at| at.is_string()));
                });
                build
            };
            assert_eq!(
                changed_build().await,
                test_case.expected_before_plan,
                "{}: unknown before the location plans it",
                test_case.description
            );
            let mut feed =
                opened(request_person_feed(&client, &api, (location, "development", &admin)).await)
                    .await;
            let report = |generation: i64, changed: &[&str]| ChangedBuildReport {
                session_id,
                generation,
                changed: changed.iter().map(|key| (*key).to_owned()).collect(),
                downstream: vec!["model:customers".into()],
                failing: None,
            };
            let failing = |reason: String| ChangedBuildReport {
                session_id,
                generation: 2,
                changed: Vec::new(),
                downstream: Vec::new(),
                failing: Some(reason),
            };
            report_changed_build(
                store,
                (location, "development"),
                &report(2, &["model:orders"]),
            )
            .await
            .expect("changed build");
            assert_eq!(
                next_notice(&mut feed).await.expect("node status"),
                json!({"topic": "node_status:development", "version": 2, "previous_version": 1, "full": true}),
                "{}: a new changed build tells clients to read the states again",
                test_case.description
            );
            assert_eq!(
                changed_build().await,
                json!({"generation": 2, "changed": ["model:orders"], "downstream": ["model:customers"]}),
                "{}: served with the node states",
                test_case.description
            );
            report_changed_build(
                store,
                (location, "development"),
                &failing("Model orders cannot plan a full rebuild".into()),
            )
            .await
            .expect("failing changed build");
            next_notice(&mut feed).await.expect("node status");
            assert_eq!(
                changed_build().await,
                json!({"generation": 2, "changed": [], "downstream": [], "failing": "Model orders cannot plan a full rebuild"}),
                "{}: a failed plan replaces the count with its reason",
                test_case.description
            );
            report_location_health(
                store,
                location,
                &LocationHealthReport {
                    event_id: Uuid::now_v7(),
                    session_id,
                    target: "development".into(),
                    health: LocationHealth::Connection {
                        generation: 2,
                        healthy: true,
                        trigger: None,
                        elapsed_millis: None,
                    },
                },
            )
            .await
            .expect("published");
            let health = location_health(store, (location, Some("development")))
                .await
                .expect("health");
            assert_eq!(
                (
                    health
                        .iter()
                        .find_map(|observation| match &observation.report.health {
                            LocationHealth::ChangePlanFailed { generation, reason } => {
                                Some((*generation, reason.clone()))
                            }
                            _ => None,
                        }),
                    LocationState::of(&health, ("development", chrono::Utc::now())),
                ),
                (
                    Some((2, "Model orders cannot plan a full rebuild".to_owned())),
                    LocationState::ChangePlanFailed
                ),
                "{}: a failed plan of what changed is the location's health, not only a log line",
                test_case.description
            );
            assert_eq!(
                next_topic(&mut feed).await,
                Some(format!("location_health:{location}")),
                "{}: a location's health report reaches the people following its target",
                test_case.description
            );
            let partial = ChangedBuildReport {
                session_id,
                generation: 2,
                changed: vec!["model:totals".into()],
                downstream: Vec::new(),
                failing: Some(
                    "Can't plan 1 model: orders: model 'orders' sets full_refresh false".into(),
                ),
            };
            report_changed_build(store, (location, "development"), &partial)
                .await
                .expect("partial changed build");
            next_notice(&mut feed).await.expect("node status");
            assert_eq!(
                changed_build().await,
                json!({"generation": 2, "changed": ["model:totals"], "downstream": [], "failing": "Can't plan 1 model: orders: model 'orders' sets full_refresh false"}),
                "{}: a plan with models it couldn't plan keeps the rest and names those models",
                test_case.description
            );
            assert_eq!(
                LocationState::of(
                    &location_health(store, (location, Some("development")))
                        .await
                        .expect("health"),
                    ("development", chrono::Utc::now())
                ),
                LocationState::ChangePlanFailed,
                "{}: models it couldn't plan count as a failed plan of what changed",
                test_case.description
            );
            assert!(
                matches!(
                    report_changed_build(
                        store,
                        (location, "development"),
                        &failing("x".repeat(2_001))
                    )
                    .await,
                    Err(ControlError::Invalid(_))
                ),
                "{}: an unbounded reason is refused",
                test_case.description
            );
            let refused = [
                report_changed_build(
                    store,
                    (location, "development"),
                    &report(1, &["model:orders"]),
                )
                .await,
                report_changed_build(store, (location, "development"), &report(2, &["orders"]))
                    .await,
            ];
            assert!(
                matches!(
                    refused,
                    [Err(ControlError::Fenced), Err(ControlError::Invalid(_))]
                ),
                "{}: an older generation is fenced and nodes must be keys: {refused:?}",
                test_case.description
            );
            let mut republished = input.clone();
            republished.generation = 3;
            publish_snapshot(store, &republished)
                .await
                .expect("new snapshot");
            assert_eq!(
                changed_build().await,
                Value::Null,
                "{}: a new snapshot's changed build is unknown until the location plans it",
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

#[test]
fn given_held_node_states_when_a_signal_names_their_target_then_only_watched_targets_stay_held() {
    let test_cases = [
        HeldTargetTestCase {
            description: "a target nobody watches is dropped and read again on the next request",
            watched: false,
            pending: (false, false),
            expected_held_and_refreshed: (false, false),
        },
        HeldTargetTestCase {
            description: "a watched target stays held and refreshes",
            watched: true,
            pending: (false, false),
            expected_held_and_refreshed: (true, true),
        },
        HeldTargetTestCase {
            description: "an unwatched target with a refresh pending stays held until it finishes",
            watched: false,
            pending: (false, true),
            expected_held_and_refreshed: (true, false),
        },
        HeldTargetTestCase {
            description: "an unwatched target with another refresh pass due stays held and refreshes",
            watched: false,
            pending: (true, false),
            expected_held_and_refreshed: (true, true),
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
            let mut input = orders_snapshot(location, &["orders"]);
            input.generation = 2;
            publish_snapshot(store, &input).await.expect("orders");
            let changes = tokio::sync::broadcast::channel(16).0;
            let (cache, task) = start_node_states(store, &changes);
            let task = tokio::spawn(task);
            let scope = (location, "development");
            let watch = test_case.watched.then(|| watch_target(&cache, scope));
            current_view(&cache, scope, None).await.expect("first view");
            let entry = target_cache(&cache, scope);
            let (dirty, scheduled) = test_case.pending;
            entry
                .dirty
                .store(dirty, std::sync::atomic::Ordering::SeqCst);
            entry
                .scheduled
                .store(scheduled, std::sync::atomic::Ordering::SeqCst);
            let mut updates = cache.updates.subscribe();
            changes
                .send(ChangeMessage::Changed {
                    location_id: location,
                    target_hash: sha256_hex(b"development"),
                    topic: ChangeTopic::NodeStatus,
                })
                .expect("signal");
            let refreshed = tokio::time::timeout(Duration::from_secs(2), updates.recv())
                .await
                .is_ok();
            let held = cache
                .targets
                .lock()
                .expect("targets")
                .contains_key(&(location, "development".to_owned()));
            assert_eq!(
                (held, refreshed),
                test_case.expected_held_and_refreshed,
                "{}",
                test_case.description
            );
            drop(watch);
            task.abort();
            close_test_store(fixture).await;
        });
    }
}
