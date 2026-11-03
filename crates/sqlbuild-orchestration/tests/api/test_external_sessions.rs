//! Real HTTP federation sessions retain live roles, usable-admin protection and audited revocation.

use std::collections::{BTreeMap, BTreeSet};

use reqwest::{Client, StatusCode, header};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::api::models::{ApiAuthentication, TrustedProxyAuthentication};
use crate::api::tests::helpers::{start_api_with_authentication, stop_api};
use crate::api::tests::test_types::ExternalSessionTestCase;
use crate::authorization::main::bootstrap_user::bootstrap_user;
use crate::credentials::main::{
    identities::create_principal::create_principal, locations::create_location::create_location,
    sessions::set_password::set_password, sessions::verify_password::verify_password,
};
use crate::credentials::models::{AuditContext, PasswordAuthority, PrincipalKind};
use crate::store::main::{
    begin::begin, commit::commit, migrate_store::migrate_store, query::query,
};
use crate::store::models::{Parameter, TransactionMode};
use crate::store::tests::fault_helpers::{install_fault, remove_fault};
use crate::store::tests::fault_types::TransactionFault;
use crate::store::tests::replica_helpers::second_store;
use crate::store::tests::store_helpers::{
    close_store, close_test_store, open_test_store, reconstruct_warehouse_predecessor, run,
};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_federated_people_when_administrators_and_sessions_change_then_live_authority_and_lifetime_hold()
 {
    let test_cases = [
        ExternalSessionTestCase {
            description: "SQLite federated administrator/session journey",
            backend: Backend::Sqlite,
            expected_lifetime: 3600,
        },
        ExternalSessionTestCase {
            description: "PostgreSQL federated administrator/session journey",
            backend: Backend::Postgres,
            expected_lifetime: 3600,
        },
    ];
    for test_case in test_cases {
        run(async {
            let fixture = open_test_store(test_case.backend).await.expect("database");
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let replica = second_store(store).await;
            let context = AuditContext {
                actor: "local-control-store-owner".into(),
                granting_role: "control-store-owner".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../benchmarks/duckshop/edge_cases/orchestration/proxy_identities.json");
            let inputs: Value =
                serde_json::from_str(&std::fs::read_to_string(path).expect("DuckShop federation"))
                    .expect("inputs");
            let people = &inputs["administrator_journey"];
            let first_subject = people["bound"][0].as_str().expect("first bound person");
            let second_subject = people["bound"][1].as_str().expect("second bound person");
            let first = bootstrap_user(store, &context, first_subject)
                .await
                .expect("first admin");
            let second = create_principal(store, &context, (second_subject, PrincipalKind::Person))
                .await
                .expect("second person");
            let unbound = create_principal(
                store,
                &context,
                (
                    people["unbound_password_person"]
                        .as_str()
                        .expect("unbound password person"),
                    PrincipalKind::Person,
                ),
            )
            .await
            .expect("unbound person");
            set_password(
                store,
                &context,
                PasswordAuthority::Operator(unbound),
                "products-password-2026",
            )
            .await
            .expect("unbound password");
            reconstruct_warehouse_predecessor(store).await;
            let mut transaction = begin(store, TransactionMode::Write)
                .await
                .expect("populated predecessor");
            for sql in [
                "DROP TABLE sqb_external_sessions",
                "DELETE FROM sqb_schema_migrations WHERE name='external_sessions'",
            ] {
                (transaction, _) = query(transaction, sql, vec![])
                    .await
                    .expect("reconstruct immediate predecessor");
            }
            commit(transaction).await.expect("predecessor committed");
            migrate_store(&replica)
                .await
                .expect("additive session upgrade");
            migrate_store(store)
                .await
                .expect("idempotent session upgrade");
            assert!(
                verify_password(store, unbound, "products-password-2026")
                    .await
                    .expect("preserved credential")
                    .is_some()
            );
            let origin = inputs["origin"].as_str().expect("origin");
            let settings = TrustedProxyAuthentication {
                origin: origin.into(),
                trusted_peers: BTreeSet::from(["127.0.0.1".parse().expect("peer")]),
                identities: BTreeMap::from([
                    (first_subject.into(), first),
                    (second_subject.into(), second),
                ]),
            };
            let api = start_api_with_authentication(
                store,
                ApiAuthentication {
                    trusted_proxy: Some(settings.clone()),
                    ..Default::default()
                },
            )
            .await;
            let other = start_api_with_authentication(
                &replica,
                ApiAuthentication {
                    trusted_proxy: Some(settings),
                    ..Default::default()
                },
            )
            .await;
            let client = Client::builder()
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .expect("client");
            let login = |url: &str, subject: &str| {
                client
                    .get(format!("{url}/api/auth/proxy"))
                    .header(header::HOST, "control.example.com")
                    .header("x-sqb-user", subject)
            };
            for condition in [
                "WHEN NEW.action='session.created'",
                "WHEN NEW.action='proxy.authenticated'",
            ] {
                let fault = TransactionFault {
                    description: "failed federation publication",
                    table: "sqb_security_audit",
                    operation: "INSERT",
                    condition,
                    sqlite_body: "SELECT RAISE(ABORT, 'synthetic session audit failure')",
                    postgres_body: "RAISE EXCEPTION 'synthetic session audit failure'",
                };
                install_fault(store, &fault).await;
                let response = login(&api.url, first_subject)
                    .send()
                    .await
                    .expect("failed session publication");
                assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
                assert!(!response.headers().contains_key(header::SET_COOKIE));
                remove_fault(store, &fault).await;
            }
            let mut sessions = Vec::new();
            for subject in [first_subject, second_subject] {
                let response = login(&api.url, subject).send().await.expect("SSO login");
                assert_eq!(response.status(), StatusCode::OK);
                let cookie = response
                    .headers()
                    .get_all(header::SET_COOKIE)
                    .iter()
                    .map(|value| value.to_str().expect("cookie"))
                    .find(|value| value.starts_with("__Host-sqb-session="))
                    .expect("session cookie")
                    .split(';')
                    .next()
                    .expect("cookie pair")
                    .to_owned();
                let body = response.json::<Value>().await.expect("session response");
                sessions.push((
                    cookie,
                    body["csrf_token"].as_str().expect("csrf").to_owned(),
                    body["session_id"]
                        .as_str()
                        .expect("id")
                        .parse::<Uuid>()
                        .expect("session UUID"),
                ));
            }
            let request = |index: usize, path: &str| {
                client
                    .post(format!("{}{path}", other.url))
                    .header(header::HOST, "control.example.com")
                    .header(header::ORIGIN, origin)
                    .header(header::COOKIE, &sessions[index].0)
                    .header("x-sqb-csrf", &sessions[index].1)
            };
            let read = |index: usize, path: &str| {
                client
                    .get(format!("{}{path}", other.url))
                    .header(header::HOST, "control.example.com")
                    .header(header::COOKIE, &sessions[index].0)
            };
            let roles = |principal_id| json!({"principal_id":principal_id,"role":"admin","location_id":null,"target":null});
            for id in [unbound, second] {
                let response = request(0, "/api/admin/roles")
                    .json(&roles(id))
                    .send()
                    .await
                    .expect("admin grant");
                assert_eq!(response.status(), StatusCode::OK);
                assert_eq!(
                    response.json::<Value>().await.expect("applied grant")["applied"],
                    true
                );
            }
            // A bound person needs no local password; an unbound password account cannot replace it.
            assert_eq!(
                request(0, &format!("/api/admin/people/{first}"))
                    .json(&"revoke")
                    .send()
                    .await
                    .expect("SSO admin replacement")
                    .status(),
                StatusCode::OK
            );
            assert_eq!(
                request(0, "/api/admin/roles")
                    .json(&roles(first))
                    .send()
                    .await
                    .expect("live revoked admin")
                    .status(),
                StatusCode::FORBIDDEN
            );
            for operation in ["revoke", "disable"] {
                let response = request(1, &format!("/api/admin/people/{second}"))
                    .json(&operation)
                    .send()
                    .await
                    .expect("final usable SSO admin");
                assert_eq!(response.status(), StatusCode::FORBIDDEN);
                let body = response.json::<Value>().await.expect("invariant denial");
                assert!(body["error"].as_str().expect("error").starts_with(
                    "cannot remove or disable the final usable installation administrator; decision "
                ));
            }
            let location = create_location(store, "orders", &context)
                .await
                .expect("registered location");
            let metadata = format!("/api/control/locations/{location}/targets/development/runs");
            assert_eq!(
                read(0, &metadata)
                    .send()
                    .await
                    .expect("no ambient role")
                    .status(),
                StatusCode::FORBIDDEN
            );
            let viewer = json!({"principal_id":first,"role":"viewer","location_id":location,"target":"development"});
            assert_eq!(
                request(1, "/api/admin/roles")
                    .json(&viewer)
                    .send()
                    .await
                    .expect("viewer grant")
                    .status(),
                StatusCode::OK
            );
            assert_eq!(
                read(0, &metadata)
                    .send()
                    .await
                    .expect("live viewer")
                    .status(),
                StatusCode::OK
            );
            let mut revocation = viewer;
            revocation["revoke"] = json!(true);
            assert_eq!(
                request(1, "/api/admin/roles")
                    .json(&revocation)
                    .send()
                    .await
                    .expect("viewer revocation")
                    .status(),
                StatusCode::OK
            );
            assert_eq!(
                read(0, &metadata)
                    .send()
                    .await
                    .expect("live viewer loss")
                    .status(),
                StatusCode::FORBIDDEN
            );
            let (transaction, rows) = query(
                begin(store, TransactionMode::Read)
                    .await
                    .expect("inspect sessions"),
                "SELECT session_id,created_at,expires_at FROM sqb_external_sessions",
                vec![],
            )
            .await
            .expect("committed sessions only");
            assert_eq!(rows.len(), 2, "failed audits did not create sessions");
            for row in rows {
                let created: chrono::DateTime<chrono::Utc> =
                    serde_json::from_value(row["created_at"].clone()).expect("created");
                let expires: chrono::DateTime<chrono::Utc> =
                    serde_json::from_value(row["expires_at"].clone()).expect("expiry");
                // Session expiry is truncated to database microsecond precision.
                assert!(
                    (test_case.expected_lifetime * 1_000_000 - 1
                        ..=test_case.expected_lifetime * 1_000_000)
                        .contains(
                            &(expires - created)
                                .num_microseconds()
                                .expect("bounded lifetime")
                        ),
                    "{}",
                    test_case.description
                );
            }
            commit(transaction).await.expect("inspection committed");
            let (transaction, _) = query(
                begin(store, TransactionMode::Write)
                    .await
                    .expect("expire session"),
                "UPDATE sqb_external_sessions SET expires_at=created_at WHERE session_id=$1",
                vec![Parameter::Id(sessions[0].2)],
            )
            .await
            .expect("clock boundary");
            commit(transaction).await.expect("expiry committed");
            assert_eq!(
                read(0, "/api/auth/session")
                    .send()
                    .await
                    .expect("expired session")
                    .status(),
                StatusCode::UNAUTHORIZED
            );
            set_password(
                store,
                &context,
                PasswordAuthority::Operator(second),
                "customers-password-2026",
            )
            .await
            .expect("owner credential rotation");
            assert_eq!(
                read(1, "/api/auth/session")
                    .send()
                    .await
                    .expect("SSO session revoked on reset")
                    .status(),
                StatusCode::UNAUTHORIZED
            );
            assert_eq!(
                login(&api.url, second_subject)
                    .send()
                    .await
                    .expect("fresh SSO after reset")
                    .status(),
                StatusCode::OK
            );
            stop_api(other).await;
            stop_api(api).await;
            close_store(replica).await;
            close_test_store(fixture).await;
        });
    }
}
