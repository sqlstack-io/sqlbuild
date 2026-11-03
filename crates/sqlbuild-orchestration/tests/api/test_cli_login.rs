//! Real cross-replica exchange, atomic audit rollback and live password/session revocation.

use futures_util::future::join_all;
use reqwest::{Client, StatusCode};
use serde_json::{Value, json};
use sqlbuild_recall::records::main::sha256_hex::sha256_hex;
use uuid::Uuid;

use crate::api::models::ApiAuthentication;
use crate::api::tests::helpers::{start_api_with_authentication, stop_api};
use crate::api::tests::test_types::CliLoginTestCase;
use crate::credentials::main::{
    identities::create_principal::create_principal, sessions::login_password::login_password,
    sessions::revoke_session::revoke_session, sessions::set_password::set_password,
};
use crate::credentials::models::{AuditContext, PasswordAuthority, PasswordLogin, PrincipalKind};
use crate::store::main::{
    begin::begin, commit::commit, migrate_store::migrate_store, query::query,
};
use crate::store::models::{Parameter, TransactionMode};
use crate::store::tests::fault_helpers::{install_fault, remove_fault};
use crate::store::tests::fault_types::TransactionFault;
use crate::store::tests::logical_state::logical_state;
use crate::store::tests::replica_helpers::second_store;
use crate::store::tests::store_helpers::{close_store, close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_browser_approved_cli_login_when_exchange_races_or_authority_changes_then_credentials_stay_single_use_atomic_and_live()
 {
    let test_cases = [
        CliLoginTestCase {
            description: "SQLite single-use browser handoff",
            backend: Backend::Sqlite,
            expected_exchanges: 1,
        },
        CliLoginTestCase {
            description: "PostgreSQL single-use browser handoff",
            backend: Backend::Postgres,
            expected_exchanges: 1,
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
                .join("../../benchmarks/duckshop/edge_cases/orchestration/cli_login.json");
            let input: Value =
                serde_json::from_str(&std::fs::read_to_string(path).expect("fixture"))
                    .expect("inputs");
            let password = input["password"].as_str().expect("password");
            let replacement = input["replacement"].as_str().expect("replacement");
            let origin = input["origin"].as_str().expect("origin");
            let host = origin.strip_prefix("https://").expect("HTTPS");
            let context = AuditContext {
                actor: "test-admin".into(),
                granting_role: "admin".into(),
                request_id: Uuid::now_v7(),
                ip_address: Some("127.0.0.1".parse().expect("peer")),
            };
            let person =
                create_principal(store, &context, ("orders-reader", PrincipalKind::Person))
                    .await
                    .expect("person");
            set_password(
                store,
                &context,
                PasswordAuthority::Operator(person),
                password,
            )
            .await
            .expect("password");
            let PasswordLogin::Authenticated(browser) =
                login_password(store, &context, ("orders-reader", password))
                    .await
                    .expect("browser login")
            else {
                panic!("browser credentials");
            };
            let authentication = || ApiAuthentication {
                password_origin: Some(origin.into()),
                ..ApiAuthentication::default()
            };
            let first = start_api_with_authentication(store, authentication()).await;
            let second = start_api_with_authentication(&replica, authentication()).await;
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert("host", host.parse().expect("host"));
            headers.insert("origin", origin.parse().expect("origin"));
            let client = Client::builder()
                .default_headers(headers)
                .build()
                .expect("client");
            let secret = input["client_secret"]
                .as_str()
                .expect("synthetic exchange secret");
            let started = client
                .post(format!("{}/api/auth/cli/start", first.url))
                .json(&json!({"client_hash":sha256_hex(secret.as_bytes())}))
                .send()
                .await
                .expect("start");
            assert_eq!(started.status(), StatusCode::OK);
            assert_eq!(started.headers()["cache-control"], "no-store");
            let flow: Value = started.json().await.expect("handoff");
            let code = flow["browser_code"].as_str().expect("browser code");
            let flow_id = code.split_once('.').expect("flow ID").0;
            let exchange = json!({"flow_id":flow_id,"client_secret":secret});
            assert_eq!(
                client
                    .post(format!("{}/api/auth/cli/claim", second.url))
                    .json(&exchange)
                    .send()
                    .await
                    .expect("pending")
                    .status(),
                StatusCode::ACCEPTED
            );
            let approval = json!({"browser_code":code,"approve":true});
            let cookie = format!("__Host-sqb-session={}", browser.bearer);
            assert_eq!(
                client
                    .post(format!("{}/api/auth/cli/approve", second.url))
                    .header("cookie", &cookie)
                    .json(&approval)
                    .send()
                    .await
                    .expect("missing CSRF")
                    .status(),
                StatusCode::UNAUTHORIZED
            );
            let cancelled: Value = client
                .post(format!("{}/api/auth/cli/start", first.url))
                .json(&json!({"client_hash":sha256_hex(secret.as_bytes())}))
                .send()
                .await
                .expect("cancel start")
                .json()
                .await
                .expect("cancel flow");
            let cancelled_code = cancelled["browser_code"].as_str().expect("cancel code");
            assert_eq!(
                client
                    .post(format!("{}/api/auth/cli/approve", second.url))
                    .header("cookie", &cookie)
                    .header("x-sqb-csrf", &browser.csrf)
                    .json(&json!({"browser_code":cancelled_code,"approve":false}))
                    .send()
                    .await
                    .expect("cancel")
                    .status(),
                StatusCode::NO_CONTENT
            );
            assert_eq!(client.post(format!("{}/api/auth/cli/claim", first.url))
                .json(&json!({"flow_id":cancelled_code.split_once('.').expect("cancel ID").0,"client_secret":secret}))
                .send().await.expect("cancel claim").status(), StatusCode::UNAUTHORIZED);
            let expired: Value = client
                .post(format!("{}/api/auth/cli/start", first.url))
                .json(&json!({"client_hash":sha256_hex(secret.as_bytes())}))
                .send()
                .await
                .expect("expiry start")
                .json()
                .await
                .expect("expiry flow");
            let expired_code = expired["browser_code"].as_str().expect("expiry code");
            let expired_id =
                Uuid::parse_str(expired_code.split_once('.').expect("expiry ID").0).expect("ID");
            let transaction = begin(store, TransactionMode::Write)
                .await
                .expect("expiry fence");
            let expired_at = transaction.now - chrono::Duration::seconds(1);
            let (transaction, _) = query(
                transaction,
                "UPDATE sqb_cli_login_flows SET expires_at=$2 WHERE flow_id=$1",
                vec![Parameter::Id(expired_id), Parameter::Time(expired_at)],
            )
            .await
            .expect("elapsed expiry");
            commit(transaction).await.expect("commit expiry");
            assert_eq!(
                client
                    .post(format!("{}/api/auth/cli/approve", second.url))
                    .header("cookie", &cookie)
                    .header("x-sqb-csrf", &browser.csrf)
                    .json(&json!({"browser_code":expired_code,"approve":true}))
                    .send()
                    .await
                    .expect("expired approval")
                    .status(),
                StatusCode::UNAUTHORIZED
            );
            assert_eq!(
                client
                    .post(format!("{}/api/auth/cli/claim", first.url))
                    .json(&json!({"flow_id":expired_id,"client_secret":secret}))
                    .send()
                    .await
                    .expect("expired claim")
                    .status(),
                StatusCode::UNAUTHORIZED
            );
            let approved = client
                .post(format!("{}/api/auth/cli/approve", second.url))
                .header("cookie", &cookie)
                .header("x-sqb-csrf", &browser.csrf)
                .json(&approval)
                .send()
                .await
                .expect("approve");
            assert_eq!(approved.status(), StatusCode::NO_CONTENT);
            assert!(!approved.headers().contains_key("set-cookie"));
            assert!(approved.bytes().await.expect("approval body").is_empty());
            let wrong = json!({"flow_id":flow_id,"client_secret":"not-the-cli-secret"});
            assert_eq!(
                client
                    .post(format!("{}/api/auth/cli/claim", first.url))
                    .json(&wrong)
                    .send()
                    .await
                    .expect("wrong exchange")
                    .status(),
                StatusCode::UNAUTHORIZED
            );
            let fault = TransactionFault {
                description: "CLI exchange audit rollback",
                table: "sqb_security_audit",
                operation: "INSERT",
                condition: "WHEN NEW.action='cli.login.exchanged'",
                sqlite_body: "SELECT RAISE(ABORT, 'CLI exchange audit failed')",
                postgres_body: "RAISE EXCEPTION 'CLI exchange audit failed'",
            };
            install_fault(store, &fault).await;
            let before = logical_state(store).await;
            assert_eq!(
                client
                    .post(format!("{}/api/auth/cli/claim", first.url))
                    .json(&exchange)
                    .send()
                    .await
                    .expect("failed exchange")
                    .status(),
                StatusCode::INTERNAL_SERVER_ERROR
            );
            assert_eq!(
                logical_state(store).await,
                before,
                "failed exchange preserves every logical row"
            );
            remove_fault(store, &fault).await;
            let attempts = join_all((0..4).map(|index| {
                let client = &client;
                let exchange = &exchange;
                let url = [&first.url, &second.url][index % 2];
                async move {
                    client
                        .post(format!("{url}/api/auth/cli/claim"))
                        .json(exchange)
                        .send()
                        .await
                        .expect("concurrent exchange")
                }
            }))
            .await;
            assert_eq!(
                attempts
                    .iter()
                    .filter(|response| response.status() == StatusCode::OK)
                    .count(),
                test_case.expected_exchanges,
                "{}",
                test_case.description
            );
            assert_eq!(
                attempts
                    .iter()
                    .filter(|response| response.status() == StatusCode::UNAUTHORIZED)
                    .count(),
                3
            );
            let issued: Value = attempts
                .into_iter()
                .find(|response| response.status() == StatusCode::OK)
                .expect("one credential")
                .json()
                .await
                .expect("CLI credential");
            assert_eq!(issued["identity"]["principal_id"], person.to_string());
            let authorization = format!("Session {}", issued["bearer"].as_str().expect("bearer"));
            for url in [&first.url, &second.url] {
                assert_eq!(
                    client
                        .get(format!("{url}/api/auth/session"))
                        .header("authorization", &authorization)
                        .send()
                        .await
                        .expect("CLI reload transport")
                        .status(),
                    StatusCode::OK
                );
                assert_eq!(
                    client
                        .get(format!("{url}/api/control/audit"))
                        .header("authorization", &authorization)
                        .send()
                        .await
                        .expect("no implicit grant")
                        .status(),
                    StatusCode::FORBIDDEN
                );
                assert_eq!(
                    client
                        .post(format!("{url}/api/auth/logout"))
                        .header("authorization", &authorization)
                        .send()
                        .await
                        .expect("CLI CSRF required")
                        .status(),
                    StatusCode::UNAUTHORIZED
                );
            }
            assert_eq!(
                client
                    .get(format!("{}/api/auth/session", first.url))
                    .header("authorization", &authorization)
                    .header("cookie", &cookie)
                    .send()
                    .await
                    .expect("ambiguous transport")
                    .status(),
                StatusCode::UNAUTHORIZED
            );
            let started: Value = client
                .post(format!("{}/api/auth/cli/start", first.url))
                .json(&json!({"client_hash":sha256_hex(secret.as_bytes())}))
                .send()
                .await
                .expect("next start")
                .json()
                .await
                .expect("next flow");
            let next_code = started["browser_code"].as_str().expect("next code");
            assert_eq!(
                client
                    .post(format!("{}/api/auth/cli/approve", first.url))
                    .header("cookie", &cookie)
                    .header("x-sqb-csrf", &browser.csrf)
                    .json(&json!({"browser_code":next_code,"approve":true}))
                    .send()
                    .await
                    .expect("next approve")
                    .status(),
                StatusCode::NO_CONTENT
            );
            let (transaction, _) = revoke_session(
                begin(store, TransactionMode::Write).await.expect("write"),
                &context,
                (person, Some(browser.identity.session_id)),
            )
            .await
            .expect("revoke browser");
            commit(transaction).await.expect("commit revoke");
            assert_eq!(client.post(format!("{}/api/auth/cli/claim", second.url))
                .json(&json!({"flow_id":next_code.split_once('.').expect("next ID").0,"client_secret":secret}))
                .send().await.expect("revoked authorizer").status(), StatusCode::UNAUTHORIZED);
            set_password(
                store,
                &context,
                PasswordAuthority::Operator(person),
                replacement,
            )
            .await
            .expect("rotate password");
            for url in [&first.url, &second.url] {
                assert_eq!(
                    client
                        .get(format!("{url}/api/auth/session"))
                        .header("authorization", &authorization)
                        .send()
                        .await
                        .expect("live password revocation")
                        .status(),
                    StatusCode::UNAUTHORIZED
                );
            }
            stop_api(first).await;
            stop_api(second).await;
            close_store(replica).await;
            close_test_store(fixture).await;
        });
    }
}
