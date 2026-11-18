//! A person who can see a location's runs marks an ongoing condition handled as it looks now; everyone
//! reads who did and until when, a changed condition can be acknowledged again, and the mark withdrawn.

use serde_json::{Value, json};
use uuid::Uuid;

use crate::api::models::{ApiAuthentication, HumanLogin};
use crate::api::tests::denials_helpers::person_send;
use crate::api::tests::helpers::{start_api_with_authentication, stop_api};
use crate::credentials::tests::location_helpers::create_location;
use crate::credentials::tests::role_helpers::role_fixture;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_an_ongoing_condition_when_a_person_acknowledges_it_then_everyone_reads_its_shape_until_it_changes_or_is_withdrawn()
 {
    for backend in [Backend::Sqlite, Backend::Postgres] {
        run(async {
            let Some(fixture) = open_test_store(backend).await else {
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
            let url = format!(
                "{}/api/control/locations/{inventory}/targets/development/attention-acknowledgements",
                api.url
            );
            let send = |body: Option<Value>| {
                let (url, client) = (url.clone(), client.clone());
                async move {
                    let response =
                        person_send(&client, (&url, administrator.0, administrator.1), body).await;
                    (
                        response.status().as_u16(),
                        response.json::<Value>().await.unwrap_or(Value::Null),
                    )
                }
            };
            let request = |key: &str, shape: Value, acknowledged: bool| json!({"event_id": Uuid::now_v7(), "key": key, "shape": shape, "acknowledged": acknowledged});
            let one_source = json!({"members": ["raw__payments:warn"]});
            let acknowledge = request("stale_sources", one_source.clone(), true);

            let first = send(Some(acknowledge.clone())).await;
            let retried = send(Some(acknowledge.clone())).await;
            let mut conflicting = acknowledge.clone();
            conflicting["shape"] = json!({"members": []});
            let reused = send(Some(conflicting)).await;
            let again = send(Some(request("stale_sources", one_source.clone(), true))).await;
            let bad_key = send(Some(request("", json!({}), true))).await;
            let bad_shape = send(Some(request(
                "stale_sources",
                json!(["not", "an", "object"]),
                true,
            )))
            .await;
            let large = send(Some(request(
                "stale_sources",
                json!({"members": vec!["x".repeat(64); 200]}),
                true,
            )))
            .await;
            assert_eq!(
                [
                    first.0,
                    retried.0,
                    reused.0,
                    again.0,
                    bad_key.0,
                    bad_shape.0,
                    large.0
                ],
                [200, 200, 400, 200, 400, 400, 400],
                "{backend:?}: statuses"
            );
            assert_eq!(
                [
                    &first.1["applied"],
                    &retried.1["applied"],
                    &again.1["applied"]
                ],
                [&json!(true), &json!(false), &json!(false)],
                "{backend:?}: a retry, or the same shape again while it holds, changes nothing"
            );

            let listed = send(None).await.1;
            assert_eq!(
                listed.as_array().map(Vec::len),
                Some(1),
                "{backend:?}: {listed}"
            );
            assert_eq!(
                (
                    &listed[0]["key"],
                    &listed[0]["shape"],
                    &listed[0]["acknowledged_by"]
                ),
                (&json!("stale_sources"), &one_source, &json!("orders-admin"))
            );
            let at: chrono::DateTime<chrono::Utc> =
                serde_json::from_value(listed[0]["acknowledged_at"].clone()).expect("time");
            let until: chrono::DateTime<chrono::Utc> =
                serde_json::from_value(listed[0]["until"].clone()).expect("until");
            assert_eq!(
                until - at,
                chrono::Duration::hours(24),
                "{backend:?}: the snooze"
            );

            // Worse, then acknowledged again as it looks now.
            let two_sources = json!({"members": ["raw__payments:warn", "raw__refunds:error"]});
            let worse = send(Some(request("stale_sources", two_sources.clone(), true))).await;
            let other = send(Some(request(
                "event:tick_overdue:segments_hourly",
                json!({"code": "tick_overdue"}),
                true,
            )))
            .await;
            let listed = send(None).await.1;
            assert_eq!(
                (worse.1["applied"].clone(), other.1["applied"].clone()),
                (json!(true), json!(true))
            );
            assert_eq!(
                listed
                    .as_array()
                    .expect("listing")
                    .iter()
                    .map(|a| (a["key"].clone(), a["shape"].clone()))
                    .collect::<Vec<_>>(),
                [
                    (
                        json!("event:tick_overdue:segments_hourly"),
                        json!({"code": "tick_overdue"})
                    ),
                    (json!("stale_sources"), two_sources)
                ],
                "{backend:?}: the latest shape per condition"
            );

            let withdrawn = send(Some(request("stale_sources", json!({}), false))).await;
            let withdrawn_again = send(Some(request("stale_sources", json!({}), false))).await;
            let listed = send(None).await.1;
            assert_eq!(
                (
                    withdrawn.1["applied"].clone(),
                    withdrawn_again.1["applied"].clone()
                ),
                (json!(true), json!(false))
            );
            assert_eq!(
                listed
                    .as_array()
                    .expect("listing")
                    .iter()
                    .map(|a| a["key"].clone())
                    .collect::<Vec<_>>(),
                [json!("event:tick_overdue:segments_hourly")]
            );

            // Another target's conditions are its own.
            let elsewhere = person_send(
                &client,
                (&format!("{}/api/control/locations/{inventory}/targets/production/attention-acknowledgements", api.url), administrator.0, administrator.1),
                None,
            )
            .await;
            let elsewhere = elsewhere.json::<Value>().await.unwrap_or(Value::Null);
            assert!(
                elsewhere.as_array().is_none_or(Vec::is_empty),
                "{backend:?}: {elsewhere}"
            );
            stop_api(api).await;
            close_test_store(fixture).await;
        });
    }
}
