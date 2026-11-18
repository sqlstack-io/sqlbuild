//! A person marks an ongoing condition handled as it looks now, until it changes or is withdrawn.

use serde_json::{Value, json};
use uuid::Uuid;

use crate::acknowledgements::models::AttentionSnooze;
use crate::api::models::ServerFacts;
use crate::api::models::{ApiAuthentication, HumanLogin};
use crate::api::tests::denials_helpers::person_send;
use crate::api::tests::helpers::{start_api_with_authentication, start_api_with_facts, stop_api};
use crate::api::tests::test_types::{
    AttentionAcknowledgementTestCase, AttentionSnoozeTestCase, AutomaticWithdrawalTestCase,
    NamedWithdrawalTestCase, SnoozeParseTestCase,
};
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
                listed["acknowledgements"]
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
            let listed = listed["acknowledgements"].clone();
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
            let elsewhere = send("production", None).await.1["acknowledgements"].clone();
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

#[test]
fn given_a_configured_snooze_when_a_condition_is_acknowledged_then_it_lapses_unchanged_only_after_that_long()
 {
    let test_cases = [
        AttentionSnoozeTestCase {
            description: "the default, 24 hours",
            snooze: AttentionSnooze::default(),
            expected_snooze_seconds: json!(86_400),
            expected_until_seconds: Some(86_400),
            expected_same_shape_applies_after_a_wait: false,
        },
        AttentionSnoozeTestCase {
            description: "one second",
            snooze: AttentionSnooze::parse("1s").expect("one second"),
            expected_snooze_seconds: json!(1),
            expected_until_seconds: Some(1),
            expected_same_shape_applies_after_a_wait: true,
        },
        AttentionSnoozeTestCase {
            description: "no time limit",
            snooze: AttentionSnooze::parse("0").expect("no limit"),
            expected_snooze_seconds: Value::Null,
            expected_until_seconds: None,
            expected_same_shape_applies_after_a_wait: false,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(Backend::Sqlite).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let roles = role_fixture(store).await;
            let inventory = create_location(store, "inventory", "orders-admin")
                .await
                .expect("location");
            let api = start_api_with_facts(
                store,
                ApiAuthentication {
                    login: HumanLogin::Password("https://control.example.com".into()),
                    ..ApiAuthentication::default()
                },
                ServerFacts {
                    attention_snooze: test_case.snooze,
                    ..ServerFacts::default()
                },
            )
            .await;
            let client = reqwest::Client::new();
            let url = format!(
                "{}/api/control/locations/{inventory}/targets/development/attention-acknowledgements",
                api.url
            );
            let administrator = (
                url.as_str(),
                roles.administrator.bearer.as_str(),
                roles.administrator.csrf.as_str(),
            );
            let acknowledge = || json!({"event_id": Uuid::now_v7(), "key": "stale_models", "shape": {"counts": {"changed": 2}}, "acknowledged": true});
            person_send(&client, administrator, Some(acknowledge())).await;
            let listed: Value = person_send(&client, administrator, None)
                .await
                .json()
                .await
                .expect("listing");
            tokio::time::sleep(std::time::Duration::from_millis(1_100)).await;
            let again: Value = person_send(&client, administrator, Some(acknowledge()))
                .await
                .json()
                .await
                .expect("again");
            let ack = &listed["acknowledgements"][0];
            let until_seconds = ack["until"].as_str().map(|until| {
                let at: chrono::DateTime<chrono::Utc> =
                    serde_json::from_value(ack["acknowledged_at"].clone()).expect("time");
                let until: chrono::DateTime<chrono::Utc> = until.parse().expect("until");
                (until - at).num_seconds()
            });
            assert_eq!(
                (&listed["snooze_seconds"], until_seconds, &again["applied"]),
                (
                    &test_case.expected_snooze_seconds,
                    test_case.expected_until_seconds,
                    &json!(test_case.expected_same_shape_applies_after_a_wait)
                ),
                "{}",
                test_case.description
            );
            stop_api(api).await;
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_a_snooze_setting_when_it_is_parsed_then_durations_and_zero_are_accepted_and_the_rest_rejected()
 {
    let test_cases = [
        SnoozeParseTestCase {
            description: "hours",
            text: "24h",
            expected_seconds: Ok(Some(86_400)),
        },
        SnoozeParseTestCase {
            description: "days and hours",
            text: "1d12h",
            expected_seconds: Ok(Some(129_600)),
        },
        SnoozeParseTestCase {
            description: "minutes",
            text: "90m",
            expected_seconds: Ok(Some(5_400)),
        },
        SnoozeParseTestCase {
            description: "no time limit",
            text: "0",
            expected_seconds: Ok(None),
        },
        SnoozeParseTestCase {
            description: "no unit",
            text: "24",
            expected_seconds: Err("attention_snooze must be a duration"),
        },
        SnoozeParseTestCase {
            description: "a word",
            text: "a day",
            expected_seconds: Err("attention_snooze must be a duration"),
        },
        SnoozeParseTestCase {
            description: "a year",
            text: "365d",
            expected_seconds: Ok(Some(31_536_000)),
        },
        SnoozeParseTestCase {
            description: "longer than a year",
            text: "366d",
            expected_seconds: Err("attention_snooze can be at most 365 days"),
        },
        SnoozeParseTestCase {
            description: "months",
            text: "1mo",
            expected_seconds: Err("can't use years or months"),
        },
        SnoozeParseTestCase {
            description: "too long",
            text: "99999999999999999999d",
            expected_seconds: Err("attention_snooze must be a duration"),
        },
    ];
    for test_case in test_cases {
        let expected_error = test_case.expected_seconds.err();
        let outcome = AttentionSnooze::parse(test_case.text)
            .map(AttentionSnooze::seconds)
            .map_err(|error| {
                expected_error.is_some_and(|expected| error.to_string().contains(expected))
            });
        assert_eq!(
            outcome,
            test_case.expected_seconds.map_err(|_| true),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_cleared_condition_when_viewers_withdraw_it_automatically_then_it_is_withdrawn_once_with_the_reason()
 {
    let test_cases = [
        AutomaticWithdrawalTestCase {
            description: "SQLite",
            backend: Backend::Sqlite,
            expected_applied: [true, false],
            expected_reason: "automatic: the condition cleared",
            expected_withdrawals: 1,
        },
        AutomaticWithdrawalTestCase {
            description: "PostgreSQL",
            backend: Backend::Postgres,
            expected_applied: [true, false],
            expected_reason: "automatic: the condition cleared",
            expected_withdrawals: 1,
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
            let url = format!(
                "{}/api/control/locations/{inventory}/targets/development/attention-acknowledgements",
                api.url
            );
            let administrator = (
                url.as_str(),
                roles.administrator.bearer.as_str(),
                roles.administrator.csrf.as_str(),
            );
            let shape = json!({"counts": {"changed": 2}});
            person_send(
                &client,
                administrator,
                Some(json!({"event_id": Uuid::now_v7(), "key": "stale_models", "shape": shape, "acknowledged": true})),
            )
            .await;
            let withdrawal = json!({"event_id": Uuid::now_v7(), "key": "stale_models", "shape": shape, "acknowledged": false, "reason": test_case.expected_reason});
            let mut applied = Vec::new();
            for _viewer in 0..2 {
                let response: Value = person_send(&client, administrator, Some(withdrawal.clone()))
                    .await
                    .json()
                    .await
                    .expect("withdrawal");
                applied.push(response["applied"].clone());
            }
            let listed: Value = person_send(&client, administrator, None)
                .await
                .json()
                .await
                .expect("listing");
            let audit: Value = person_send(
                &client,
                (
                    &format!("{}/api/control/audit?limit=50", api.url),
                    administrator.1,
                    administrator.2,
                ),
                None,
            )
            .await
            .json()
            .await
            .expect("audit");
            let withdrawals: Vec<Value> = audit
                .as_array()
                .or_else(|| audit["events"].as_array())
                .into_iter()
                .flatten()
                .filter(|event| event["action"] == "attention.acknowledgement_withdrawn")
                .map(|event| event["scope_json"]["reason"].clone())
                .collect();
            assert_eq!(
                (applied, listed["acknowledgements"].clone(), withdrawals),
                (
                    test_case.expected_applied.map(Value::Bool).to_vec(),
                    json!([]),
                    vec![json!(test_case.expected_reason); test_case.expected_withdrawals]
                ),
                "{}",
                test_case.description
            );
            stop_api(api).await;
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_a_withdrawal_naming_an_acknowledgement_when_a_newer_one_stands_then_it_changes_nothing() {
    let test_cases = [
        NamedWithdrawalTestCase {
            description: "SQLite",
            backend: Backend::Sqlite,
            expected_statuses: [200, 200, 200, 400, 200],
            expected_applied: [false, true],
            expected_standing_after_stale: "re-acknowledged",
        },
        NamedWithdrawalTestCase {
            description: "PostgreSQL",
            backend: Backend::Postgres,
            expected_statuses: [200, 200, 200, 400, 200],
            expected_applied: [false, true],
            expected_standing_after_stale: "re-acknowledged",
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
            let url = format!(
                "{}/api/control/locations/{inventory}/targets/development/attention-acknowledgements",
                api.url
            );
            let administrator = (
                url.as_str(),
                roles.administrator.bearer.as_str(),
                roles.administrator.csrf.as_str(),
            );
            let send = |body: Value| {
                let client = client.clone();
                async move {
                    let response = person_send(&client, administrator, Some(body)).await;
                    let status = response.status().as_u16();
                    (
                        status,
                        response.json::<Value>().await.unwrap_or(Value::Null),
                    )
                }
            };
            let first = Uuid::now_v7();
            let again = Uuid::now_v7();
            let acknowledged = send(json!({"event_id": first, "key": "stale_models", "shape": {"counts": {"changed": 2}}, "acknowledged": true})).await;
            let reacknowledged = send(json!({"event_id": again, "key": "stale_models", "shape": {"counts": {"changed": 3}}, "acknowledged": true, "reason": "re-acknowledged"})).await;
            let stale = send(json!({"event_id": Uuid::now_v7(), "key": "stale_models", "shape": {}, "acknowledged": false, "reason": "automatic: the condition cleared", "withdraws": first})).await;
            let named_acknowledgement = send(json!({"event_id": Uuid::now_v7(), "key": "stale_models", "shape": {}, "acknowledged": true, "withdraws": first})).await;
            let listed: Value = person_send(&client, administrator, None)
                .await
                .json()
                .await
                .expect("listing");
            let current = send(json!({"event_id": Uuid::now_v7(), "key": "stale_models", "shape": {}, "acknowledged": false, "reason": "automatic: the condition cleared", "withdraws": again})).await;
            assert_eq!(
                (
                    [
                        acknowledged.0,
                        reacknowledged.0,
                        stale.0,
                        named_acknowledgement.0,
                        current.0
                    ],
                    [&stale.1["applied"], &current.1["applied"]],
                    (
                        &listed["acknowledgements"][0]["event_id"],
                        &listed["acknowledgements"][0]["shape"]["counts"]["changed"]
                    ),
                ),
                (
                    test_case.expected_statuses,
                    test_case.expected_applied.map(Value::Bool).each_ref(),
                    (&json!(again), &json!(3)),
                ),
                "{}: {}",
                test_case.description,
                test_case.expected_standing_after_stale
            );
            stop_api(api).await;
            close_test_store(fixture).await;
        });
    }
}
