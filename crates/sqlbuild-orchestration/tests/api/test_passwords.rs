//! Real HTTP sessions use secure cookies, exact origins and live cross-replica credentials.

use reqwest::header::{COOKIE, HOST, ORIGIN, SET_COOKIE};
use reqwest::{Client, StatusCode};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::api::models::{
    ApiAuthentication, AuthenticationDenial, HumanLogin, PasswordSessionResponse,
};
use crate::api::tests::helpers::{start_api_with_authentication, stop_api};
use crate::api::tests::test_types::PasswordApiTestCase;
use crate::credentials::main::{
    identities::create_principal::create_principal, sessions::set_password::set_password,
};
use crate::credentials::models::{AuditContext, DefaultRole, PasswordAuthority};
use crate::store::main::{
    begin::begin, commit::commit, migrate_store::migrate_store, query::query,
};
use crate::store::models::TransactionMode;
use crate::store::tests::replica_helpers::second_store;
use crate::store::tests::store_helpers::{close_store, close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_password_http_sessions_when_changed_or_revoked_then_cookie_and_csrf_boundaries_hold_across_replicas()
 {
    let test_cases = [
        PasswordApiTestCase {
            description: "SQLite browser session contract",
            backend: Backend::Sqlite,
            expected_lifetime: 3600,
        },
        PasswordApiTestCase {
            description: "PostgreSQL browser session contract",
            backend: Backend::Postgres,
            expected_lifetime: 3600,
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
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
                "../../benchmarks/duckshop/edge_cases/orchestration/password_credentials.json",
            );
            let cases: Value =
                serde_json::from_str(&std::fs::read_to_string(path).expect("DuckShop fixture"))
                    .expect("session inputs");
            let username = cases["username"].as_str().expect("username");
            let maximum_password_bytes = cases["maximum_password_bytes"]
                .as_u64()
                .expect("password byte limit") as usize;
            let password = cases["http_password_character"]
                .as_str()
                .expect("password character")
                .repeat(maximum_password_bytes);
            let replacement = cases["http_replacement_character"]
                .as_str()
                .expect("replacement character")
                .repeat(maximum_password_bytes);
            let password = password.as_str();
            let replacement = replacement.as_str();
            let origin = cases["origin"].as_str().expect("origin");
            let context = AuditContext {
                actor: "test-admin".into(),
                granting_role: "admin".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            let person = create_principal(store, &context, username)
                .await
                .expect("person");
            set_password(
                store,
                &context,
                PasswordAuthority::Operator(person),
                password,
            )
            .await
            .expect("set password");
            let api = start_api_with_authentication(
                store,
                ApiAuthentication {
                    login: HumanLogin::Password(origin.into()),
                    ..ApiAuthentication::default()
                },
            )
            .await;
            let other = start_api_with_authentication(
                &replica,
                ApiAuthentication {
                    login: HumanLogin::Password(origin.into()),
                    default_role: DefaultRole::Viewer,
                    ..ApiAuthentication::default()
                },
            )
            .await;
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert(
                HOST,
                origin
                    .strip_prefix("https://")
                    .expect("HTTPS origin")
                    .parse()
                    .expect("host"),
            );
            headers.insert(ORIGIN, origin.parse().expect("origin"));
            let client = Client::builder()
                .default_headers(headers)
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .expect("browser transport");
            let response = client
                .post(format!("{}/api/auth/password/login", api.url))
                .json(&json!({"username": username, "password": "x".repeat(16 * 1024)}))
                .send()
                .await
                .expect("oversized request");
            assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
            let mut decisions = Vec::new();
            for invalid_origin in cases["invalid_origins"]
                .as_array()
                .expect("invalid origins")
            {
                let response = client
                    .post(format!("{}/api/auth/password/login", api.url))
                    .header(ORIGIN, invalid_origin.as_str().expect("origin"))
                    .json(&json!({"username": username, "password": password}))
                    .send()
                    .await
                    .expect("origin probe");
                assert_eq!(
                    response.status(),
                    StatusCode::UNAUTHORIZED,
                    "{}",
                    test_case.description
                );
                decisions.push(
                    response
                        .json::<AuthenticationDenial>()
                        .await
                        .expect("denial decision")
                        .decision_id,
                );
            }
            for (name, secret) in [
                (username, cases["wrong"].as_str().expect("wrong password")),
                ("unknown-person", password),
            ] {
                let response = client
                    .post(format!("{}/api/auth/password/login", api.url))
                    .json(&json!({"username": name, "password": secret}))
                    .send()
                    .await
                    .expect("credential probe");
                assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
                let denial: AuthenticationDenial = response.json().await.expect("generic denial");
                assert_eq!(denial.error, "authentication rejected");
                decisions.push(denial.decision_id);
            }
            let response = client
                .post(format!("{}/api/auth/password/login", api.url))
                .json(&json!({"username": username, "password": password}))
                .send()
                .await
                .expect("login");
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(response.headers()["cache-control"], "no-store");
            let cookies: Vec<_> = response
                .headers()
                .get_all(SET_COOKIE)
                .iter()
                .map(|value| value.to_str().expect("cookie").to_owned())
                .collect();
            assert_eq!(cookies.len(), 2);
            let session = cookies
                .iter()
                .find(|cookie| cookie.starts_with("__Host-sqb-session="))
                .expect("session cookie");
            assert!(session.contains("; HttpOnly"));
            for cookie in &cookies {
                assert!(cookie.contains("; Secure; SameSite=Strict;"));
                assert!(cookie.contains("; Path=/;"));
                assert!(cookie.contains(&format!("Max-Age={}", test_case.expected_lifetime)));
                assert!(!cookie.to_lowercase().contains("domain="));
            }
            let cookie = session.split(';').next().expect("cookie pair").to_owned();
            let bearer = cookie.split_once('=').expect("session bearer").1;
            let body: Value = response.json().await.expect("login body");
            assert!(body.get("bearer").is_none());
            assert!(!body.to_string().contains(bearer));
            let issued: PasswordSessionResponse =
                serde_json::from_value(body).expect("typed session");
            assert_eq!(issued.identity.principal_id, person);
            assert!(!format!("{issued:?}").contains(&issued.csrf_token));
            let forged = format!(
                "__Host-sqb-session={}.{}",
                issued.identity.session_id,
                "0".repeat(64)
            );
            for invalid_cookie in [&forged, &format!("{cookie}; {cookie}")] {
                let response = client
                    .get(format!("{}/api/auth/session", api.url))
                    .header(COOKIE, invalid_cookie)
                    .send()
                    .await
                    .expect("invalid session cookie");
                assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
                decisions.push(
                    response
                        .json::<AuthenticationDenial>()
                        .await
                        .expect("session decision")
                        .decision_id,
                );
            }
            let response = client
                .get(format!("{}/api/auth/session", other.url))
                .header(COOKIE, &cookie)
                .send()
                .await
                .expect("second replica read");
            assert_eq!(response.status(), StatusCode::OK);
            let location_id = Uuid::now_v7();
            for (server, expected_status) in
                [(&api, StatusCode::FORBIDDEN), (&other, StatusCode::OK)]
            {
                let url = format!(
                    "{}/api/control/locations/{location_id}/targets/development/runs",
                    server.url
                );
                assert_eq!(
                    client
                        .get(&url)
                        .send()
                        .await
                        .expect("unauthenticated read")
                        .status(),
                    StatusCode::UNAUTHORIZED,
                    "viewer default cannot replace authentication"
                );
                let response = client
                    .get(url)
                    .header(COOKIE, &cookie)
                    .send()
                    .await
                    .expect("live server-owned default role");
                assert_eq!(response.status(), expected_status);
                assert_eq!(
                    response
                        .json::<Value>()
                        .await
                        .expect("metadata response")
                        .as_array()
                        .map(Vec::len),
                    (expected_status == StatusCode::OK).then_some(0),
                    "only the configured viewer receives the empty metadata array"
                );
                let response = client
                    .post(format!("{}/api/control/runs", server.url))
                    .header(COOKIE, &cookie)
                    .header("x-sqb-csrf", &issued.csrf_token)
                    .json(
                        &json!({"run_id": Uuid::now_v7(), "location_id": location_id,
                        "target": "development", "job_key": "daily_orders",
                        "reason": "Build orders", "priority": 0}),
                    )
                    .send()
                    .await
                    .expect("default role cannot execute");
                assert_eq!(response.status(), StatusCode::FORBIDDEN);
            }
            for csrf in cases["invalid_csrf"].as_array().expect("invalid CSRF") {
                let response = client
                    .post(format!("{}/api/auth/logout", other.url))
                    .header(COOKIE, &cookie)
                    .header("x-sqb-csrf", csrf.as_str().expect("CSRF value"))
                    .send()
                    .await
                    .expect("CSRF probe");
                assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
                decisions.push(
                    response
                        .json::<AuthenticationDenial>()
                        .await
                        .expect("CSRF decision")
                        .decision_id,
                );
            }
            let response = client
                .get(format!(
                    "{}/api/locations/{}/targets/development/runs",
                    api.url,
                    Uuid::now_v7()
                ))
                .header(COOKIE, &cookie)
                .send()
                .await
                .expect("session is not location authority");
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
            let response = client
                .post(format!("{}/api/auth/password", other.url))
                .header(COOKIE, &cookie)
                .header("x-sqb-csrf", &issued.csrf_token)
                .json(&json!({"current_password": password, "new_password": replacement}))
                .send()
                .await
                .expect("password replacement");
            assert_eq!(response.status(), StatusCode::NO_CONTENT);
            for value in response.headers().get_all(SET_COOKIE) {
                assert!(
                    value
                        .to_str()
                        .expect("expired cookie")
                        .contains("Max-Age=0")
                );
            }
            for server in [&api, &other] {
                let response = client
                    .get(format!("{}/api/auth/session", server.url))
                    .header(COOKIE, &cookie)
                    .send()
                    .await
                    .expect("old session check");
                assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
                decisions.push(
                    response
                        .json::<AuthenticationDenial>()
                        .await
                        .expect("revocation decision")
                        .decision_id,
                );
            }
            let response = client
                .post(format!("{}/api/auth/password/login", api.url))
                .json(&json!({"username": username, "password": replacement}))
                .send()
                .await
                .expect("replacement login");
            assert_eq!(response.status(), StatusCode::OK);
            let cookie = response
                .headers()
                .get_all(SET_COOKIE)
                .iter()
                .map(|value| value.to_str().expect("cookie"))
                .find(|value| value.starts_with("__Host-sqb-session="))
                .expect("new session")
                .split(';')
                .next()
                .expect("cookie pair")
                .to_owned();
            let renewed: PasswordSessionResponse =
                response.json().await.expect("new session identity");
            assert_ne!(renewed.identity.session_id, issued.identity.session_id);
            let response = client
                .post(format!("{}/api/auth/logout", other.url))
                .header(COOKIE, &cookie)
                .header("x-sqb-csrf", &renewed.csrf_token)
                .send()
                .await
                .expect("logout");
            assert_eq!(response.status(), StatusCode::NO_CONTENT);
            let response = client
                .get(format!("{}/api/auth/session", api.url))
                .header(COOKIE, &cookie)
                .send()
                .await
                .expect("logout across replicas");
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
            decisions.push(
                response
                    .json::<AuthenticationDenial>()
                    .await
                    .expect("logout decision")
                    .decision_id,
            );
            let (transaction, rows) = query(begin(store, TransactionMode::Read).await.expect("audit read"),
                "SELECT event_id, action, ip_address, scope_json, before_json, after_json FROM sqb_security_audit", vec![]).await.expect("security evidence");
            commit(transaction).await.expect("audit read complete");
            for decision in decisions {
                assert!(
                    rows.iter().all(|row| row["event_id"] != json!(decision)),
                    "anonymous denials stay out of the durable audit"
                );
            }
            let audit = serde_json::to_string(&rows).expect("audit");
            for secret in [password, replacement, bearer, &issued.csrf_token] {
                assert!(
                    !audit.contains(secret),
                    "security audits exclude secret material"
                );
            }
            stop_api(other).await;
            stop_api(api).await;
            close_store(replica).await;
            close_test_store(fixture).await;
        });
    }
}
