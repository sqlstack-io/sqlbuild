//! People cannot start runs under the name-based IDs the server derives for its own runs.

use serde_json::{Value, json};
use uuid::{Builder, Uuid};

use crate::api::models::{ApiAuthentication, HumanLogin};
use crate::api::tests::denials_helpers::{job, person_send};
use crate::api::tests::helpers::{start_api_with_authentication, stop_api};
use crate::api::tests::test_types::RunIdRefusalTestCase;
use crate::credentials::tests::role_helpers::role_fixture;
use crate::runs::main::request_job_run::request_job_run;
use crate::runs::models::JobRunRequest;
use crate::runs::tests::helpers::snapshot;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_a_name_based_run_id_when_a_person_requests_a_run_rerun_or_adhoc_run_then_it_is_refused() {
    let test_cases = [
        RunIdRefusalTestCase {
            description: "SQLite run IDs",
            backend: Backend::Sqlite,
            expected_refusal_status: 400,
            expected_random_status: 200,
        },
        RunIdRefusalTestCase {
            description: "PostgreSQL run IDs",
            backend: Backend::Postgres,
            expected_refusal_status: 400,
            expected_random_status: 200,
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
            let mut published = snapshot(roles.location_id);
            published.generation = 2;
            published.documents.insert(
                "jobs".into(),
                json!({"orders": {"description": "Build orders", "selection": ["orders"]}}),
            );
            publish_snapshot(store, &published).await.expect("snapshot");
            let source = JobRunRequest {
                run_id: Uuid::now_v7(),
                location_id: roles.location_id,
                target: "development".into(),
                job_key: "orders".into(),
                reason: "Refresh orders".into(),
                priority: 0,
                window: None,
                full_refresh: false,
                select: Vec::new(),
                exclude: Vec::new(),
            };
            assert!(
                request_job_run(store, (&roles.context, None), &source)
                    .await
                    .expect("source run")
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
            let session = (
                roles.administrator.bearer.as_str(),
                roles.administrator.csrf.as_str(),
            );
            let derived = Uuid::new_v5(&source.run_id, b"orders");
            let mut job_run = job(roles.location_id, "orders");
            job_run["run_id"] = json!(derived);
            let requests = [
                ("runs", job_run),
                (
                    "runs/adhoc",
                    json!({"run_id": derived, "location_id": roles.location_id,
                        "target": "development", "command": ["build", "--select", "orders"],
                        "reason": "Refresh orders", "priority": 0}),
                ),
                (
                    "runs/rerun",
                    json!({"run_id": derived, "source_run_id": source.run_id,
                        "from_failure": false, "reason": "Retry orders", "priority": 0}),
                ),
            ];
            let mut refusals = Vec::new();
            for (path, body) in requests {
                let url = format!("{}/api/control/{path}", api.url);
                let response = person_send(&client, (&url, session.0, session.1), Some(body)).await;
                let status = response.status().as_u16();
                let body: Value = response.json().await.expect("JSON refusal");
                refusals.push((path, status, body["error"].clone()));
            }
            let reason = json!(format!(
                "run_id {derived} is a name-based UUID (version 5), which the server reserves for \
                 the IDs it derives for group members and retries; send a random (version 4) or \
                 time-ordered (version 7) UUID"
            ));
            assert_eq!(
                refusals,
                vec![
                    ("runs", test_case.expected_refusal_status, reason.clone()),
                    (
                        "runs/adhoc",
                        test_case.expected_refusal_status,
                        reason.clone()
                    ),
                    ("runs/rerun", test_case.expected_refusal_status, reason),
                ],
                "{}: every person run request refuses a name-based run ID",
                test_case.description
            );
            let mut random = job(roles.location_id, "orders");
            random["run_id"] = json!(Builder::from_random_bytes([7; 16]).into_uuid());
            let url = format!("{}/api/control/runs", api.url);
            let response = person_send(&client, (&url, session.0, session.1), Some(random)).await;
            assert_eq!(
                (
                    response.status().as_u16(),
                    response.json::<Value>().await.expect("JSON")
                ),
                (test_case.expected_random_status, json!({"applied": true})),
                "{}: a random run ID is still accepted",
                test_case.description
            );
            stop_api(api).await;
            close_test_store(fixture).await;
        });
    }
}
