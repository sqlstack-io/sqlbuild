//! Signed HTTPS provider exchanges exercise PKCE, browser binding and replica-safe one-use state.

use std::collections::BTreeMap;

use reqwest::{StatusCode, header};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::api::models::{ApiAuthentication, HumanLogin, OidcAuthentication};
use crate::api::tests::helpers::{start_api_with_authentication, stop_api};
use crate::api::tests::oidc_helpers::{callback, login, start_provider, stop_provider};
use crate::api::tests::oidc_test_types::{BrowserFlow, ProviderFault};
use crate::api::tests::test_types::{OidcApiTestCase, OidcThrottleTestCase};
use crate::credentials::main::identities::create_principal::create_principal;
use crate::credentials::models::AuditContext;
use crate::store::main::{
    begin::begin, commit::commit, migrate_store::migrate_store, query::query,
};
use crate::store::models::{Parameter, TransactionMode};
use crate::store::tests::fault_helpers::{install_fault, remove_fault};
use crate::store::tests::fault_types::TransactionFault;
use crate::store::tests::replica_helpers::second_store;
use crate::store::tests::store_helpers::{close_store, close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_oidc_codes_when_verified_across_replicas_then_only_bound_live_people_authenticate() {
    let test_cases = [
        OidcApiTestCase {
            description: "SQLite OIDC boundary",
            backend: Backend::Sqlite,
            expected_authentications: 5,
        },
        OidcApiTestCase {
            description: "PostgreSQL OIDC boundary",
            backend: Backend::Postgres,
            expected_authentications: 5,
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
            let provider = start_provider().await;
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../benchmarks/duckshop/edge_cases/orchestration/oidc_identities.json");
            let inputs: Value = serde_json::from_str(
                &std::fs::read_to_string(path).expect("DuckShop OIDC fixture"),
            )
            .expect("DuckShop OIDC cases");
            let context = AuditContext {
                actor: "local-control-store-owner".into(),
                granting_role: "control-store-owner".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            let person = create_principal(store, &context, "orders-person")
                .await
                .expect("person");
            let settings = OidcAuthentication {
                origin: inputs["origin"].as_str().expect("origin").into(),
                issuer: provider.state.issuer.clone(),
                client_id: "orders-control".into(),
                client_secret_env: None,
                certificate_authority: Some(provider.authority.clone()),
                identities: BTreeMap::from([
                    ("orders-person".into(), person),
                    ("absent-person".into(), Uuid::now_v7()),
                ]),
            };
            let api = start_api_with_authentication(
                store,
                ApiAuthentication {
                    login: HumanLogin::Oidc(settings.clone()),
                    ..Default::default()
                },
            )
            .await;
            let other = start_api_with_authentication(
                &replica,
                ApiAuthentication {
                    login: HumanLogin::Oidc(settings),
                    ..Default::default()
                },
            )
            .await;
            let flow = login(&provider, &api.url).await;
            let mut wrong = BrowserFlow {
                cookie: "__Host-sqb-oidc=wrong-browser".into(),
                callback: flow.callback.clone(),
            };
            assert_eq!(
                callback(&provider, &other.url, &wrong).await.status(),
                StatusCode::UNAUTHORIZED
            );
            wrong.cookie = flow.cookie.clone();
            wrong
                .callback
                .query_pairs_mut()
                .clear()
                .append_pair("state", "wrong-state")
                .append_pair("code", "wrong-code");
            assert_eq!(
                callback(&provider, &other.url, &wrong).await.status(),
                StatusCode::UNAUTHORIZED
            );
            let response = callback(&provider, &other.url, &flow).await;
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
            assert!(
                response
                    .headers()
                    .get_all(header::SET_COOKIE)
                    .iter()
                    .any(|value| value
                        .to_str()
                        .expect("cookie")
                        .contains("__Host-sqb-oidc=;"))
            );
            let cookie = response
                .headers()
                .get_all(header::SET_COOKIE)
                .iter()
                .map(|value| value.to_str().expect("cookie"))
                .find(|value| value.starts_with("__Host-sqb-session="))
                .expect("SSO session cookie")
                .split(';')
                .next()
                .expect("cookie pair")
                .to_owned();
            let body = response
                .json::<Value>()
                .await
                .expect("authenticated session");
            assert_eq!(body["principal_id"], json!(person));
            assert!(body.get("bearer").is_none());
            for url in [&api.url, &other.url] {
                let session = provider
                    .client
                    .get(format!("{url}/api/auth/session"))
                    .header(header::HOST, "control.example.com")
                    .header(header::COOKIE, &cookie)
                    .send()
                    .await
                    .expect("OIDC session shared across replicas");
                assert_eq!(session.status(), StatusCode::OK);
                assert_eq!(
                    session.json::<Value>().await.expect("live identity")["principal_id"],
                    json!(person)
                );
                assert_eq!(
                    provider
                        .client
                        .get(format!(
                            "{url}/api/control/locations/{}/targets/development/runs",
                            Uuid::now_v7()
                        ))
                        .header(header::HOST, "control.example.com")
                        .header(header::COOKIE, &cookie)
                        .send()
                        .await
                        .expect("no implicit role")
                        .status(),
                    StatusCode::FORBIDDEN
                );
            }
            assert_eq!(
                callback(&provider, &api.url, &flow).await.status(),
                StatusCode::UNAUTHORIZED
            );
            for fault in inputs["invalid_tokens"].as_array().expect("token cases") {
                *provider.state.fault.lock().expect("fault") =
                    serde_json::from_value(fault.clone()).expect("token fault");
                let flow = login(&provider, &api.url).await;
                let response = callback(&provider, &other.url, &flow).await;
                assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{fault}");
                let body = response.text().await.expect("sanitized denial");
                assert!(!body.contains("orders-access"));
                assert!(!body.contains("code_verifier"));
            }
            *provider.state.fault.lock().expect("fault") = ProviderFault::default();
            for subject in inputs["ineligible_subjects"].as_array().expect("subjects") {
                *provider.state.subject.lock().expect("subject") =
                    subject.as_str().expect("subject name").into();
                let flow = login(&provider, &api.url).await;
                assert_eq!(
                    callback(&provider, &other.url, &flow).await.status(),
                    StatusCode::UNAUTHORIZED,
                    "{subject}"
                );
            }
            *provider.state.subject.lock().expect("subject") = "orders-person".into();
            let flow = login(&provider, &api.url).await;
            *provider.state.published_key.lock().expect("published key") = "customers".into();
            provider.state.fault.lock().expect("fault").signing_key = Some("customers".into());
            assert_eq!(
                callback(&provider, &other.url, &flow).await.status(),
                StatusCode::OK,
                "a key rotated after authorization is fetched at callback"
            );
            *provider.state.published_key.lock().expect("published key") = "orders".into();
            *provider.state.fault.lock().expect("fault") = ProviderFault::default();
            for (action, condition, expected_retry) in [
                (
                    "oidc.flow.consumed",
                    "WHEN NEW.action='oidc.flow.consumed'",
                    StatusCode::OK,
                ),
                (
                    "oidc.authenticated",
                    "WHEN NEW.action='oidc.authenticated'",
                    StatusCode::UNAUTHORIZED,
                ),
            ] {
                let flow = login(&provider, &api.url).await;
                let point = TransactionFault {
                    description: "OIDC audit publication failure",
                    table: "sqb_security_audit",
                    operation: "INSERT",
                    condition,
                    sqlite_body: "SELECT RAISE(ABORT, 'synthetic OIDC audit failure')",
                    postgres_body: "RAISE EXCEPTION 'synthetic OIDC audit failure'",
                };
                install_fault(store, &point).await;
                assert_eq!(
                    callback(&provider, &other.url, &flow).await.status(),
                    StatusCode::INTERNAL_SERVER_ERROR
                );
                remove_fault(store, &point).await;
                let retry = callback(&provider, &api.url, &flow).await;
                assert_eq!(retry.status(), expected_retry, "{action}");
            }
            let flow = login(&provider, &api.url).await;
            assert_eq!(
                callback(&provider, &other.url, &flow).await.status(),
                StatusCode::OK,
                "fresh login recovers after an audited-identity publication failure"
            );
            let flow = login(&provider, &api.url).await;
            let (left, right) = tokio::join!(
                callback(&provider, &api.url, &flow),
                callback(&provider, &other.url, &flow)
            );
            let mut statuses = [left.status().as_u16(), right.status().as_u16()];
            statuses.sort();
            assert_eq!(statuses, [200, 401], "one concurrent callback succeeds");
            let flow = login(&provider, &api.url).await;
            let transaction = begin(store, TransactionMode::Write).await.expect("disable");
            let now = transaction.now;
            let (transaction, _) = query(
                transaction,
                "UPDATE sqb_principals SET disabled_at=$1 WHERE principal_id=$2",
                vec![Parameter::Time(now), Parameter::Id(person)],
            )
            .await
            .expect("live disable");
            commit(transaction).await.expect("disable commit");
            for url in [&api.url, &other.url] {
                assert_eq!(
                    provider
                        .client
                        .get(format!("{url}/api/auth/session"))
                        .header(header::HOST, "control.example.com")
                        .header(header::COOKIE, &cookie)
                        .send()
                        .await
                        .expect("disabled person session")
                        .status(),
                    StatusCode::UNAUTHORIZED
                );
            }
            assert_eq!(
                callback(&provider, &other.url, &flow).await.status(),
                StatusCode::UNAUTHORIZED
            );
            let flow = login(&provider, &api.url).await;
            let transaction = begin(store, TransactionMode::Write).await.expect("expiry");
            let expired = transaction.now - chrono::Duration::minutes(10);
            let (transaction, _) = query(
                transaction,
                "UPDATE sqb_oidc_login_flows SET expires_at=$1",
                vec![Parameter::Time(expired)],
            )
            .await
            .expect("advance expiry");
            commit(transaction).await.expect("expiry commit");
            assert_eq!(
                callback(&provider, &other.url, &flow).await.status(),
                StatusCode::UNAUTHORIZED
            );
            let machine = provider
                .client
                .get(format!(
                    "{}/api/locations/{}/health",
                    api.url,
                    Uuid::now_v7()
                ))
                .header(header::HOST, "control.example.com")
                .header(header::COOKIE, &flow.cookie)
                .send()
                .await
                .expect("machine boundary");
            assert_eq!(machine.status(), StatusCode::UNAUTHORIZED);
            let transaction = begin(store, TransactionMode::Read).await.expect("evidence");
            let (transaction, rows) = query(transaction,
                "SELECT (SELECT COUNT(*) FROM sqb_principals) AS principals, (SELECT COUNT(*) FROM sqb_sessions WHERE credential_id IS NULL) AS sessions, (SELECT COUNT(*) FROM sqb_access_tokens) AS tokens, (SELECT COUNT(*) FROM sqb_security_audit WHERE action='oidc.authenticated') AS authentications",
                vec![]).await.expect("no provisioning");
            assert_eq!(
                json!(rows[0]),
                json!({"principals":1,"sessions":test_case.expected_authentications,"tokens":0,"authentications":test_case.expected_authentications}),
                "{}",
                test_case.description
            );
            commit(transaction).await.expect("inspection commit");
            stop_api(other).await;
            stop_api(api).await;
            close_store(replica).await;
            close_test_store(fixture).await;
            stop_provider(provider).await;
        });
    }
}

#[test]
fn given_oidc_login_attempts_when_alternating_replicas_then_the_peer_limit_is_shared() {
    let test_cases = [
        OidcThrottleTestCase {
            description: "SQLite shared peer limit",
            backend: Backend::Sqlite,
            expected_allowed: 20,
        },
        OidcThrottleTestCase {
            description: "PostgreSQL shared peer limit",
            backend: Backend::Postgres,
            expected_allowed: 20,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            migrate_store(&fixture.store).await.expect("migrate");
            let replica = second_store(&fixture.store).await;
            let provider = start_provider().await;
            let settings = OidcAuthentication {
                origin: "https://control.example.com".into(),
                issuer: provider.state.issuer.clone(),
                client_id: "orders-control".into(),
                client_secret_env: None,
                certificate_authority: Some(provider.authority.clone()),
                identities: BTreeMap::new(),
            };
            let api = start_api_with_authentication(
                &fixture.store,
                ApiAuthentication {
                    login: HumanLogin::Oidc(settings.clone()),
                    ..Default::default()
                },
            )
            .await;
            let other = start_api_with_authentication(
                &replica,
                ApiAuthentication {
                    login: HumanLogin::Oidc(settings),
                    ..Default::default()
                },
            )
            .await;
            let mut statuses = Vec::new();
            for url in [&api.url, &other.url].into_iter().cycle().take(22) {
                statuses.push(
                    provider
                        .client
                        .get(format!("{url}/api/auth/oidc/login"))
                        .header(header::HOST, "control.example.com")
                        .send()
                        .await
                        .expect("login attempt")
                        .status(),
                );
            }
            assert_eq!(
                &statuses[..test_case.expected_allowed],
                vec![StatusCode::SEE_OTHER; test_case.expected_allowed],
                "{}",
                test_case.description
            );
            assert_eq!(
                &statuses[test_case.expected_allowed..],
                [StatusCode::TOO_MANY_REQUESTS; 2]
            );
            stop_api(other).await;
            stop_api(api).await;
            close_store(replica).await;
            close_test_store(fixture).await;
            stop_provider(provider).await;
        });
    }
}
