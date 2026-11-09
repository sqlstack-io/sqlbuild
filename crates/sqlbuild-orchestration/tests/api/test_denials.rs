//! Refusals name no node and read the same for a forbidden object and an unknown one.

use serde_json::{Value, json};
use uuid::Uuid;

use crate::api::models::{ApiAuthentication, HumanLogin};
use crate::api::tests::denials_helpers::{audited_scope, job, masked, person_send, trigger};
use crate::api::tests::helpers::{start_api_with_authentication, stop_api};
use crate::api::tests::test_types::{DenialRequest, DenialTestCase};
use crate::credentials::tests::location_helpers::create_location;
use crate::credentials::tests::role_helpers::role_fixture;
use crate::declarations::main::list_triggers::list_triggers;
use crate::runs::main::request_job_run::request_job_run;
use crate::runs::models::JobRunRequest;
use crate::runs::tests::helpers::snapshot;
use crate::scheduler::tests::contracts::ticks::helpers::calendar_text;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

/// Location-scoped reads and the action each needs.
const READS: [(&str, &str); 13] = [
    ("runs", "view"),
    ("nodes", "view"),
    ("nodes/orders/runs", "view"),
    ("jobs", "view"),
    ("triggers", "view"),
    ("changes", "view"),
    ("catalog?schema=orders", "view"),
    ("statistics?schema=orders&name=orders", "view"),
    ("checks", "view"),
    ("checks/orders_not_null", "view"),
    ("maintenance", "view"),
    ("checks/orders_not_null/sql", "view.sql"),
    (
        "checks/orders_not_null/results/01a1b000-0000-7000-8000-000000000000/evidence",
        "view.evidence",
    ),
];

const NODES: [&str; 4] = ["customers", "inventory", "orders", "products"];

#[test]
fn given_refused_requests_when_people_send_them_then_no_node_is_named_and_unknown_objects_look_forbidden()
 {
    let test_cases = [
        DenialTestCase {
            description: "SQLite denials",
            backend: Backend::Sqlite,
            expected_status: 403,
        },
        DenialTestCase {
            description: "PostgreSQL denials",
            backend: Backend::Postgres,
            expected_status: 403,
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
            let inventory = create_location(store, "inventory", "orders-admin")
                .await
                .expect("unassigned location");
            let mut published = snapshot(inventory);
            published.documents.insert(
                "jobs".into(),
                json!({"orders": {"description": "Build orders", "selection": ["orders"]}}),
            );
            published.documents.insert("triggers".into(), json!({"daily_orders": {
                "description": "Refresh orders", "job_key": "orders", "starts_at": calendar_text(0),
                "schedule": {"kind": "calendar", "times": [calendar_text(0), "2030-01-01T00:00:00Z"],
                    "refresh_at": "2029-01-01T00:00:00Z"}}}));
            publish_snapshot(store, &published)
                .await
                .expect("unassigned snapshot");
            let trigger_id = list_triggers(store, inventory, "development")
                .await
                .expect("trigger")[0]
                .trigger_id;
            let queued = JobRunRequest {
                run_id: Uuid::now_v7(),
                location_id: inventory,
                target: "development".into(),
                job_key: "orders".into(),
                reason: "Refresh orders".into(),
                priority: 0,
                window: None,
                full_refresh: false,
            };
            assert!(
                request_job_run(store, &roles.context, &queued)
                    .await
                    .expect("queued run")
            );
            let api = start_api_with_authentication(
                store,
                ApiAuthentication {
                    login: HumanLogin::Password("https://control.example.com".into()),
                    ..ApiAuthentication::default()
                },
            )
            .await;
            let client = reqwest::Client::new();
            let member = (roles.member.bearer.as_str(), roles.member.csrf.as_str());
            let read = |location: Uuid, path: &str| {
                format!(
                    "{}/api/control/locations/{location}/targets/development/{path}",
                    api.url
                )
            };
            let runs = format!("{}/api/control/runs", api.url);
            let triggers = format!("{}/api/control/triggers", api.url);
            let cancel = format!("{}/api/control/runs/cancel", api.url);
            let rerun = format!("{}/api/control/runs/rerun", api.url);
            let unknown_run = Uuid::now_v7();
            let unknown_trigger = Uuid::now_v7();
            let mut cases: Vec<(String, &str, Vec<DenialRequest>)> = READS
                .iter()
                .map(|(path, action)| {
                    (
                        (*path).to_owned(),
                        *action,
                        vec![
                            (read(roles.location_id, path), None),
                            (read(inventory, path), None),
                            (read(Uuid::now_v7(), path), None),
                        ],
                    )
                })
                .collect();
            for part in [
                "events",
                "statements",
                "statements/sql?statement_id=s",
                "output",
            ] {
                cases.push((
                    format!("run {part}"),
                    "view",
                    vec![
                        (
                            read(inventory, &format!("runs/{}/{part}", queued.run_id)),
                            None,
                        ),
                        (read(inventory, &format!("runs/{unknown_run}/{part}")), None),
                        (
                            read(roles.location_id, &format!("runs/{unknown_run}/{part}")),
                            None,
                        ),
                    ],
                ));
            }
            cases.extend([
                (
                    "run nodes".to_owned(),
                    "view",
                    vec![
                        (
                            read(inventory, &format!("runs/{}/nodes", queued.run_id)),
                            None,
                        ),
                        (read(inventory, &format!("runs/{unknown_run}/nodes")), None),
                        (
                            read(roles.location_id, &format!("runs/{unknown_run}/nodes")),
                            None,
                        ),
                    ],
                ),
                (
                    "trigger ticks".to_owned(),
                    "view",
                    vec![
                        (
                            read(inventory, &format!("triggers/{trigger_id}/ticks")),
                            None,
                        ),
                        (
                            read(inventory, &format!("triggers/{unknown_trigger}/ticks")),
                            None,
                        ),
                        (
                            read(
                                roles.location_id,
                                &format!("triggers/{unknown_trigger}/ticks"),
                            ),
                            None,
                        ),
                    ],
                ),
                (
                    "job run".to_owned(),
                    "job.run",
                    vec![
                        (runs.clone(), Some(job(inventory, "orders"))),
                        (runs.clone(), Some(job(inventory, "returns"))),
                        (runs.clone(), Some(job(Uuid::now_v7(), "orders"))),
                    ],
                ),
                (
                    "trigger pause".to_owned(),
                    "trigger.toggle",
                    vec![
                        (triggers.clone(), Some(trigger(trigger_id, "pause"))),
                        (triggers.clone(), Some(trigger(unknown_trigger, "pause"))),
                    ],
                ),
                (
                    "sensor cursor reset".to_owned(),
                    "sensor.reset_cursor",
                    vec![
                        (triggers.clone(), Some(trigger(trigger_id, "reset_cursor"))),
                        (
                            triggers.clone(),
                            Some(trigger(unknown_trigger, "reset_cursor")),
                        ),
                    ],
                ),
                (
                    "run cancel".to_owned(),
                    "run.cancel",
                    vec![
                        (
                            cancel.clone(),
                            Some(json!({"run_id": queued.run_id, "event_id": Uuid::now_v7(),
                                "reason": "Stop orders"})),
                        ),
                        (
                            cancel.clone(),
                            Some(json!({"run_id": unknown_run, "event_id": Uuid::now_v7(),
                                "reason": "Stop orders"})),
                        ),
                    ],
                ),
                (
                    "rerun".to_owned(),
                    "run.rerun",
                    vec![
                        (
                            rerun.clone(),
                            Some(
                                json!({"run_id": Uuid::now_v7(), "source_run_id": queued.run_id,
                                "from_failure": false, "reason": "Retry orders", "priority": 0}),
                            ),
                        ),
                        (
                            rerun.clone(),
                            Some(
                                json!({"run_id": Uuid::now_v7(), "source_run_id": unknown_run,
                                "from_failure": false, "reason": "Retry orders", "priority": 0}),
                            ),
                        ),
                    ],
                ),
            ]);
            for (route, action, requests) in cases {
                let mut responses: Vec<(Value, Uuid)> = Vec::new();
                for (url, body) in requests {
                    responses.push(
                        masked(person_send(&client, (&url, member.0, member.1), body).await).await,
                    );
                }
                let first: Value =
                    serde_json::from_str(responses[0].0["body"].as_str().expect("body"))
                        .expect("JSON denial");
                assert_eq!(
                    (
                        responses[0].0["status"].clone(),
                        first["error"].clone(),
                        responses
                            .iter()
                            .all(|(response, _)| *response == responses[0].0),
                    ),
                    (
                        json!(test_case.expected_status),
                        json!(format!(
                            "missing {action} permission for this target; decision <decision>"
                        )),
                        true
                    ),
                    "{}: {route} reads the same for forbidden and unknown objects: {responses:?}",
                    test_case.description
                );
                assert!(
                    NODES.iter().all(|node| !responses[0].0["body"]
                        .as_str()
                        .unwrap_or_default()
                        .contains(node)),
                    "{}: {route} names a node",
                    test_case.description
                );
                for (_, decision) in &responses {
                    assert_ne!(
                        audited_scope(store, *decision).await,
                        Value::Null,
                        "{}: {route} audits every refusal",
                        test_case.description
                    );
                }
            }
            assert_eq!(
                audited_scope(
                    store,
                    masked(
                        person_send(
                            &client,
                            (&read(inventory, "runs"), member.0, member.1),
                            None
                        )
                        .await
                    )
                    .await
                    .1
                )
                .await,
                json!([
                    NODES,
                    "missing view on nodes customers, inventory, orders, products; all requested nodes denied"
                ]),
                "{}: the audit keeps the missing nodes",
                test_case.description
            );
            stop_api(api).await;
            close_test_store(fixture).await;
        });
    }
}
