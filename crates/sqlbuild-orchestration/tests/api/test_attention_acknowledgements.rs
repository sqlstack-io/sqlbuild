//! A person marks an ongoing condition handled as it looks now, until it changes or is withdrawn.

use serde_json::{Value, json};
use uuid::Uuid;

use crate::api::models::{ApiAuthentication, HumanLogin};
use crate::api::tests::denials_helpers::person_send;
use crate::api::tests::helpers::{start_api_with_authentication, stop_api};
use crate::api::tests::test_types::AttentionAcknowledgementTestCase;
use crate::credentials::tests::location_helpers::create_location;
use crate::credentials::tests::role_helpers::role_fixture;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_an_ongoing_condition_when_a_person_acknowledges_it_then_everyone_reads_its_shape_until_it_changes_or_is_withdrawn()
 {
    let test_cases = [
        AttentionAcknowledgementTestCase {
            description: "SQLite",
            backend: Backend::Sqlite,
            expected_statuses: [200, 200, 400, 200, 400, 400, 400],
            expected_applied: [true, false, false, true, true, true, false],
            expected_acknowledged_by: "orders-admin",
            expected_snooze_hours: 24,
            expected_keys: (
                &["event:tick_overdue:segments_hourly", "stale_sources"],
                &["event:tick_overdue:segments_hourly"],
            ),
        },
        AttentionAcknowledgementTestCase {
            description: "PostgreSQL",
            backend: Backend::Postgres,
            expected_statuses: [200, 200, 400, 200, 400, 400, 400],
            expected_applied: [true, false, false, true, true, true, false],
            expected_acknowledged_by: "orders-admin",
            expected_snooze_hours: 24,
            expected_keys: (
                &["event:tick_overdue:segments_hourly", "stale_sources"],
                &["event:tick_overdue:segments_hourly"],
            ),
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
            let url = |target: &str| {
                format!(
                    "{}/api/control/locations/{inventory}/targets/{target}/attention-acknowledgements",
                    api.url
                )
            };
            let send = |target: &str, body: Option<Value>| {
                let url = url(target);
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
            let request = |key: &str, shape: Value, acknowledged: bool| json!({"event_id": Uuid::now_v7(), "key": key, "shape": shape, "acknowledged": acknowledged});
            let keys = |listed: &Value| {
                listed
                    .as_array()
                    .map(|all| all.iter().map(|a| a["key"].clone()).collect::<Vec<_>>())
                    .unwrap_or_default()
            };
            let one_source = json!({"members": ["raw__payments:warn"]});
            let acknowledge = request("stale_sources", one_source.clone(), true);
            let mut conflicting = acknowledge.clone();
            conflicting["shape"] = json!({"members": []});

            let first = send("development", Some(acknowledge.clone())).await;
            let retried = send("development", Some(acknowledge)).await;
            let reused = send("development", Some(conflicting)).await;
            let again = send(
                "development",
                Some(request("stale_sources", one_source, true)),
            )
            .await;
            let empty_key = send("development", Some(request("", json!({}), true))).await;
            let array_shape = send(
                "development",
                Some(request("stale_sources", json!(["x"]), true)),
            )
            .await;
            let large_shape = send(
                "development",
                Some(request(
                    "stale_sources",
                    json!({"members": vec!["x".repeat(64); 200]}),
                    true,
                )),
            )
            .await;
            let listed = send("development", None).await.1;
            let at: chrono::DateTime<chrono::Utc> =
                serde_json::from_value(listed[0]["acknowledged_at"].clone()).expect("time");
            let until: chrono::DateTime<chrono::Utc> =
                serde_json::from_value(listed[0]["until"].clone()).expect("until");
            let worse = send(
                "development",
                Some(request(
                    "stale_sources",
                    json!({"members": ["raw__payments:warn", "raw__refunds:error"]}),
                    true,
                )),
            )
            .await;
            let other = send(
                "development",
                Some(request(
                    "event:tick_overdue:segments_hourly",
                    json!({"code": "tick_overdue"}),
                    true,
                )),
            )
            .await;
            let changed = send("development", None).await.1;
            let withdrawn = send(
                "development",
                Some(request("stale_sources", json!({}), false)),
            )
            .await;
            let withdrawn_again = send(
                "development",
                Some(request("stale_sources", json!({}), false)),
            )
            .await;
            let remaining = send("development", None).await.1;
            let elsewhere = send("production", None).await.1;
            assert_eq!(
                (
                    [
                        first.0,
                        retried.0,
                        reused.0,
                        again.0,
                        empty_key.0,
                        array_shape.0,
                        large_shape.0
                    ],
                    [
                        &first.1["applied"],
                        &retried.1["applied"],
                        &again.1["applied"],
                        &worse.1["applied"],
                        &other.1["applied"],
                        &withdrawn.1["applied"],
                        &withdrawn_again.1["applied"],
                    ],
                    &listed[0]["acknowledged_by"],
                    (until - at).num_hours(),
                    (keys(&changed), keys(&remaining)),
                    elsewhere,
                ),
                (
                    test_case.expected_statuses,
                    test_case.expected_applied.map(Value::Bool).each_ref(),
                    &json!(test_case.expected_acknowledged_by),
                    test_case.expected_snooze_hours,
                    (
                        test_case.expected_keys.0.iter().map(|k| json!(k)).collect(),
                        test_case.expected_keys.1.iter().map(|k| json!(k)).collect(),
                    ),
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
