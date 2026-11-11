//! A person who can see a location's runs marks one handled, everyone sees who did, and the mark can be withdrawn.

use serde_json::{Value, json};
use uuid::Uuid;

use crate::api::models::{ApiAuthentication, HumanLogin};
use crate::api::tests::denials_helpers::person_send;
use crate::api::tests::helpers::{start_api_with_authentication, stop_api};
use crate::api::tests::test_types::AcknowledgementTestCase;
use crate::credentials::tests::location_helpers::create_location;
use crate::credentials::tests::role_helpers::role_fixture;
use crate::runs::main::request_job_run::request_job_run;
use crate::runs::models::JobRunRequest;
use crate::runs::tests::helpers::snapshot;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_a_failed_run_when_a_person_acknowledges_it_then_everyone_reads_who_did_until_it_is_withdrawn()
 {
    let test_cases = [
        AcknowledgementTestCase {
            description: "SQLite",
            backend: Backend::Sqlite,
            expected_statuses: [200, 200, 400, 200],
            expected_applied: [true, false, true],
            expected_acknowledged_by: "orders-admin",
        },
        AcknowledgementTestCase {
            description: "PostgreSQL",
            backend: Backend::Postgres,
            expected_statuses: [200, 200, 400, 200],
            expected_applied: [true, false, true],
            expected_acknowledged_by: "orders-admin",
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
                .expect("location");
            let mut published = snapshot(inventory);
            published.documents.insert(
                "jobs".into(),
                json!({"orders": {"description": "Build orders", "selection": ["orders"]}}),
            );
            publish_snapshot(store, &published).await.expect("snapshot");
            let run_id = Uuid::now_v7();
            let queued = JobRunRequest {
                run_id,
                location_id: inventory,
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
                request_job_run(store, (&roles.context, None), &queued)
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
            let administrator = (
                roles.administrator.bearer.as_str(),
                roles.administrator.csrf.as_str(),
            );
            let url = |location: Uuid| {
                format!(
                    "{}/api/control/locations/{location}/targets/development/acknowledgements",
                    api.url
                )
            };
            let send = |location: Uuid, body: Option<Value>| {
                let url = url(location);
                let client = client.clone();
                async move {
                    let response =
                        person_send(&client, (&url, administrator.0, administrator.1), body).await;
                    let status = response.status().as_u16();
                    (
                        status,
                        response.json::<Value>().await.unwrap_or(Value::Null),
                    )
                }
            };
            let acknowledge =
                json!({"event_id": Uuid::now_v7(), "run_id": run_id, "acknowledged": true});
            let first = send(inventory, Some(acknowledge.clone())).await;
            let retried = send(inventory, Some(acknowledge)).await;
            let listed = send(inventory, None).await.1;
            // The run exists, but not in this location, which the administrator can also see.
            let elsewhere = send(
                roles.location_id,
                Some(json!({"event_id": Uuid::now_v7(), "run_id": run_id, "acknowledged": true})),
            )
            .await;
            let withdrawn = send(
                inventory,
                Some(json!({"event_id": Uuid::now_v7(), "run_id": run_id, "acknowledged": false})),
            )
            .await;
            let after = send(inventory, None).await.1;
            assert_eq!(
                (
                    [first.0, retried.0, elsewhere.0, withdrawn.0],
                    [
                        &first.1["applied"],
                        &retried.1["applied"],
                        &withdrawn.1["applied"]
                    ],
                    (&listed[0]["run_id"], &listed[0]["acknowledged_by"]),
                    after,
                ),
                (
                    test_case.expected_statuses,
                    test_case.expected_applied.map(Value::Bool).each_ref(),
                    (&json!(run_id), &json!(test_case.expected_acknowledged_by)),
                    json!([]),
                ),
                "{}",
                test_case.description
            );
            stop_api(api).await;
            close_test_store(fixture).await;
        });
    }
}
