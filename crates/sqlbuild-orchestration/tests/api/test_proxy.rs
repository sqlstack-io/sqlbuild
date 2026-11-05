//! Explicit proxy bindings authenticate each request without sessions, provisioning or machine authority.

use std::collections::{BTreeMap, BTreeSet};

use reqwest::{Client, StatusCode, header};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::api::models::{
    ApiAuthentication, AuthenticationDenial, HumanLogin, TrustedProxyAuthentication,
};
use crate::api::tests::helpers::{start_api_with_authentication, stop_api};
use crate::api::tests::test_types::ProxyApiTestCase;
use crate::credentials::main::identities::create_principal::create_principal;
use crate::credentials::models::AuditContext;
use crate::store::main::{
    begin::begin, commit::commit, migrate_store::migrate_store, query::query,
};
use crate::store::models::{Parameter, TransactionMode};
use crate::store::tests::replica_helpers::second_store;
use crate::store::tests::store_helpers::{close_store, close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_explicit_proxy_bindings_when_real_peers_and_live_people_are_checked_then_spoofing_and_implicit_grants_are_rejected()
 {
    let test_cases = [
        ProxyApiTestCase {
            description: "SQLite proxy identity boundary",
            backend: Backend::Sqlite,
            trusted_peer: "::ffff:127.0.0.1",
            expected_sessions: 0,
        },
        ProxyApiTestCase {
            description: "PostgreSQL proxy identity boundary",
            backend: Backend::Postgres,
            trusted_peer: "127.0.0.1",
            expected_sessions: 0,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let replica = second_store(store).await;
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../benchmarks/duckshop/edge_cases/orchestration/proxy_identities.json");
            let inputs: Value =
                serde_json::from_str(&std::fs::read_to_string(path).expect("DuckShop subjects"))
                    .expect("synthetic bindings");
            let value = |key: &str| inputs[key].as_str().expect("text input");
            let context = AuditContext {
                actor: "local-control-store-owner".into(),
                granting_role: "control-store-owner".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            let person = create_principal(store, &context, value("person"))
                .await
                .expect("known person");
            let settings = TrustedProxyAuthentication {
                origin: value("origin").into(),
                trusted_peers: BTreeSet::from([test_case.trusted_peer.parse().expect("proxy IP")]),
                identities: BTreeMap::from([
                    (value("subject").into(), person),
                    (value("absent_subject").into(), Uuid::now_v7()),
                ]),
            };
            let api = start_api_with_authentication(
                store,
                ApiAuthentication {
                    login: HumanLogin::TrustedProxy(settings.clone()),
                    ..ApiAuthentication::default()
                },
            )
            .await;
            let other = start_api_with_authentication(
                &replica,
                ApiAuthentication {
                    login: HumanLogin::TrustedProxy(settings.clone()),
                    ..ApiAuthentication::default()
                },
            )
            .await;
            let untrusted = start_api_with_authentication(
                store,
                ApiAuthentication {
                    login: HumanLogin::TrustedProxy(TrustedProxyAuthentication {
                        trusted_peers: BTreeSet::from([value("untrusted_peer")
                            .parse()
                            .expect("untrusted IP")]),
                        ..settings
                    }),
                    ..ApiAuthentication::default()
                },
            )
            .await;
            let client = Client::builder()
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .expect("HTTP client");
            let runs = format!(
                "/api/control/locations/{}/targets/development/runs",
                Uuid::now_v7()
            );
            let request = |url: &str| {
                client
                    .get(format!("{url}{runs}"))
                    .header(header::HOST, "control.example.com")
                    .header(header::ORIGIN, value("origin"))
            };
            for url in [&api.url, &other.url] {
                let response = request(url)
                    .header("x-sqb-user", value("subject"))
                    .send()
                    .await
                    .expect("per-request proxy authentication");
                assert_eq!(response.status(), StatusCode::FORBIDDEN);
                let forwarded = request(url)
                    .header("x-sqb-user", value("subject"))
                    .header(header::AUTHORIZATION, value("proxy_authorization"))
                    .send()
                    .await
                    .expect("proxy forwards its own authorization");
                assert_eq!(
                    forwarded.status(),
                    StatusCode::FORBIDDEN,
                    "{}: only a Session authorization is a CLI session",
                    test_case.description
                );
                assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
                assert!(!response.headers().contains_key(header::SET_COOKIE));
                let rejected_write = client
                    .post(format!("{url}/api/control/runs"))
                    .header(header::HOST, "control.example.com")
                    .header(header::ORIGIN, "https://other.example.com")
                    .header("x-sqb-user", value("subject"))
                    .json(&json!({}))
                    .send()
                    .await
                    .expect("cross-origin write");
                assert_eq!(rejected_write.status(), StatusCode::UNAUTHORIZED);
            }
            let mut denials = Vec::new();
            for subject in [
                "",
                value("unknown_subject"),
                value("person"),
                value("absent_subject"),
            ] {
                let response = request(&api.url)
                    .header("x-sqb-user", subject)
                    .send()
                    .await
                    .expect("unbound/ineligible subject");
                assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
                denials.push(
                    response
                        .json::<AuthenticationDenial>()
                        .await
                        .expect("durable denial")
                        .decision_id,
                );
            }
            for probe in [
                request(&api.url),
                request(&api.url)
                    .header("x-sqb-user", value("subject"))
                    .header("x-sqb-user", value("subject")),
                request(&api.url)
                    .header("x-sqb-user", value("subject"))
                    .headers(header::HeaderMap::from_iter([(
                        header::ORIGIN,
                        header::HeaderValue::from_static("https://other.example.com"),
                    )])),
                request(&api.url)
                    .header("x-sqb-user", value("subject"))
                    .headers(header::HeaderMap::from_iter([(
                        header::HOST,
                        header::HeaderValue::from_static("other.example.com"),
                    )])),
                request(&untrusted.url)
                    .header("x-sqb-user", value("subject"))
                    .header("x-forwarded-for", value("untrusted_peer"))
                    .header("forwarded", format!("for={}", value("untrusted_peer"))),
            ] {
                let response = probe.send().await.expect("spoofing probe");
                assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
                denials.push(
                    response
                        .json::<AuthenticationDenial>()
                        .await
                        .expect("spoofing denial")
                        .decision_id,
                );
            }
            let machine = client
                .get(format!(
                    "{}/api/locations/{}/health",
                    api.url,
                    Uuid::now_v7()
                ))
                .header(header::HOST, "control.example.com")
                .header("x-sqb-user", value("subject"))
                .send()
                .await
                .expect("no location authority");
            assert_eq!(machine.status(), StatusCode::UNAUTHORIZED);
            let transaction = begin(store, TransactionMode::Write)
                .await
                .expect("disable person");
            let now = transaction.now;
            let (transaction, _) = query(
                transaction,
                "UPDATE sqb_principals SET disabled_at=$1 WHERE principal_id=$2",
                vec![Parameter::Time(now), Parameter::Id(person)],
            )
            .await
            .expect("live disabled fact");
            commit(transaction).await.expect("disable committed");
            for url in [&api.url, &other.url] {
                let response = request(url)
                    .header("x-sqb-user", value("subject"))
                    .send()
                    .await
                    .expect("live person validation");
                assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
                denials.push(
                    response
                        .json::<AuthenticationDenial>()
                        .await
                        .expect("disabled denial")
                        .decision_id,
                );
            }
            let mut transaction = begin(store, TransactionMode::Read)
                .await
                .expect("inspect durable evidence");
            for decision in denials {
                let rows;
                (transaction, rows) = query(
                    transaction,
                    "SELECT event_id FROM sqb_security_audit WHERE event_id=$1",
                    vec![Parameter::Id(decision)],
                )
                .await
                .expect("denial audit");
                assert!(
                    rows.is_empty(),
                    "anonymous denials stay out of the durable audit"
                );
            }
            let (transaction, rows) = query(transaction, "SELECT (SELECT COUNT(*) FROM sqb_principals) AS principals, (SELECT COUNT(*) FROM sqb_sessions) AS sessions, (SELECT COUNT(*) FROM sqb_access_tokens) AS tokens", vec![]).await.expect("no implicit provisioning or personal tokens");
            assert_eq!(
                json!(rows[0]),
                json!({"principals": 1, "sessions": test_case.expected_sessions, "tokens": 0}),
                "{}",
                test_case.description
            );
            commit(transaction).await.expect("inspection complete");
            stop_api(untrusted).await;
            stop_api(other).await;
            stop_api(api).await;
            close_store(replica).await;
            close_test_store(fixture).await;
        });
    }
}
