//! Explicit proxy bindings authenticate known people without provisioning or machine authority.

use std::collections::{BTreeMap, BTreeSet};

use reqwest::{Client, StatusCode, header};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::api::models::{ApiAuthentication, AuthenticationDenial, TrustedProxyAuthentication};
use crate::api::tests::helpers::{start_api_with_authentication, stop_api};
use crate::api::tests::test_types::ProxyApiTestCase;
use crate::credentials::main::create_principal::create_principal;
use crate::credentials::models::{AuditContext, PrincipalKind};
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
            expected_authentications: 2,
        },
        ProxyApiTestCase {
            description: "PostgreSQL proxy identity boundary",
            backend: Backend::Postgres,
            trusted_peer: "127.0.0.1",
            expected_authentications: 2,
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
            let person =
                create_principal(store, &context, (value("person"), PrincipalKind::Person))
                    .await
                    .expect("known person");
            let service = create_principal(
                store,
                &context,
                ("orders-service", PrincipalKind::ServiceAccount),
            )
            .await
            .expect("service account");
            let settings = TrustedProxyAuthentication {
                origin: value("origin").into(),
                trusted_peers: BTreeSet::from([test_case.trusted_peer.parse().expect("proxy IP")]),
                identities: BTreeMap::from([
                    (value("subject").into(), person),
                    (value("service_subject").into(), service),
                    (value("absent_subject").into(), Uuid::now_v7()),
                ]),
            };
            let api = start_api_with_authentication(
                store,
                ApiAuthentication {
                    trusted_proxy: Some(settings.clone()),
                    ..ApiAuthentication::default()
                },
            )
            .await;
            let other = start_api_with_authentication(
                &replica,
                ApiAuthentication {
                    trusted_proxy: Some(settings.clone()),
                    ..ApiAuthentication::default()
                },
            )
            .await;
            let untrusted = start_api_with_authentication(
                store,
                ApiAuthentication {
                    trusted_proxy: Some(TrustedProxyAuthentication {
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
            let request = |url: &str| {
                client
                    .get(format!("{url}/api/auth/proxy"))
                    .header(header::HOST, "control.example.com")
                    .header(header::ORIGIN, value("origin"))
            };
            for url in [&api.url, &other.url] {
                let response = request(url)
                    .header("x-sqb-user", value("subject"))
                    .send()
                    .await
                    .expect("proxy authentication");
                assert_eq!(response.status(), StatusCode::OK);
                assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
                assert!(!response.headers().contains_key(header::SET_COOKIE));
                assert_eq!(
                    response.json::<Value>().await.expect("identity only"),
                    json!({"principal_id": person})
                );
            }
            let mut denials = Vec::new();
            for subject in [
                "",
                value("unknown_subject"),
                value("person"),
                value("service_subject"),
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
                (transaction, rows) = query(transaction, "SELECT actor, ip_address, request_id FROM sqb_security_audit WHERE event_id=$1 AND action='authentication.denied'", vec![Parameter::Id(decision)]).await.expect("denial audit");
                assert_eq!(rows.len(), 1);
                assert_eq!(rows[0]["actor"], "anonymous");
                assert_eq!(rows[0]["ip_address"], "127.0.0.1");
                assert_eq!(rows[0]["request_id"], json!(decision));
            }
            let (transaction, rows) = query(transaction, "SELECT actor, granting_role, ip_address FROM sqb_security_audit WHERE action='proxy.authenticated'", vec![]).await.expect("proxy audit");
            assert_eq!(
                rows.len(),
                test_case.expected_authentications,
                "{}",
                test_case.description
            );
            for row in rows {
                assert_eq!(row["actor"], format!("principal:{person}"));
                assert_eq!(row["granting_role"], "trusted-proxy");
                assert_eq!(row["ip_address"], "127.0.0.1");
            }
            let (transaction, rows) = query(transaction, "SELECT (SELECT COUNT(*) FROM sqb_principals) AS principals, (SELECT COUNT(*) FROM sqb_password_sessions) AS sessions, (SELECT COUNT(*) FROM sqb_access_tokens) AS tokens", vec![]).await.expect("no implicit provisioning");
            assert_eq!(
                json!(rows[0]),
                json!({"principals": 2, "sessions": 0, "tokens": 0})
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
