//! Concurrent login reservations, secret eligibility and atomic audit failures on both stores.

use chrono::Duration;
use futures_util::future::join_all;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::credentials::main::{
    audit::reserve_authentication_attempt::reserve_authentication_attempt,
    identities::create_principal::create_principal,
    sessions::authenticate_session::authenticate_session, sessions::login_password::login_password,
    sessions::revoke_session::revoke_session, sessions::set_password::set_password,
    sessions::verify_password::verify_password, tokens::authenticate_token::authenticate_token,
};
use crate::credentials::models::{
    AuditContext, AuthenticationAttempt, PasswordAuthority, PasswordLogin, SessionAccess,
};
use crate::credentials::tests::test_sessions::test_types::SessionTestCase;
use crate::store::main::{
    begin::begin, commit::commit, migrate_store::migrate_store, query::query,
};
use crate::store::models::{Parameter, TransactionMode};
use crate::store::tests::fault_helpers::{install_fault, remove_fault};
use crate::store::tests::fault_types::TransactionFault;
use crate::store::tests::replica_helpers::second_store;
use crate::store::tests::store_helpers::{close_store, close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[path = "test_types.rs"]
mod test_types;

#[test]
fn given_password_sessions_when_replicas_race_or_auditing_fails_then_throttles_and_credentials_remain_authoritative()
 {
    let test_cases = [
        SessionTestCase {
            description: "SQLite session storage contract",
            backend: Backend::Sqlite,
            expected_sessions: 5,
            expected_peer_attempts: 20,
        },
        SessionTestCase {
            description: "PostgreSQL session storage contract",
            backend: Backend::Postgres,
            expected_sessions: 5,
            expected_peer_attempts: 20,
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
                    .expect("password inputs");
            let password = cases["password"].as_str().expect("password");
            let replacement = cases["replacement"].as_str().expect("replacement");
            let context = AuditContext {
                actor: "test-admin".into(),
                granting_role: "admin".into(),
                request_id: Uuid::now_v7(),
                ip_address: Some("127.0.0.1".parse().expect("peer")),
            };
            let person = create_principal(store, &context, "orders-reader")
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
            let context = &context;
            let stores = [store, &replica];
            let outcomes = join_all((0..12).map(|index| async move {
                login_password(stores[index % 2], context, ("orders-reader", password))
                    .await
                    .expect("concurrent login")
            }))
            .await;
            assert_eq!(
                outcomes
                    .iter()
                    .filter(|result| matches!(result, PasswordLogin::Authenticated(_)))
                    .count(),
                test_case.expected_sessions,
                "{}",
                test_case.description
            );
            assert_eq!(
                outcomes
                    .iter()
                    .filter(|result| matches!(
                        result,
                        PasswordLogin::Denied {
                            throttled: true,
                            ..
                        }
                    ))
                    .count(),
                12 - test_case.expected_sessions
            );
            let retained = outcomes
                .into_iter()
                .find(|result| matches!(result, PasswordLogin::Authenticated(_)))
                .expect("successful session");
            let PasswordLogin::Authenticated(session) = retained else {
                panic!("expected an authenticated session");
            };
            for secret in [&session.bearer, &session.csrf] {
                assert!(!format!("{session:?}").contains(secret));
            }
            assert!(
                authenticate_token(&replica, &session.bearer)
                    .await
                    .expect("distinct bearer scope")
                    .is_none()
            );
            assert_eq!(
                authenticate_session(&replica, &session.bearer, SessionAccess::Read)
                    .await
                    .expect("live session"),
                Some(session.identity.clone())
            );
            let another_peer = AuditContext {
                ip_address: Some("127.0.0.2".parse().expect("other peer")),
                ..context.clone()
            };
            assert!(matches!(
                login_password(&replica, &another_peer, ("orders-reader", password))
                    .await
                    .expect("global identity throttle"),
                PasswordLogin::Denied {
                    throttled: true,
                    ..
                }
            ));
            let peer_results = join_all((0..30).map(|index| {
                let name = format!("customers-{index}");
                let context = &another_peer;
                async move {
                    reserve_authentication_attempt(
                        stores[index % 2],
                        context,
                        AuthenticationAttempt::Login(&name),
                    )
                    .await
                    .expect("shared peer reservation")
                    .1
                }
            }))
            .await;
            assert_eq!(
                peer_results.iter().filter(|allowed| **allowed).count(),
                test_case.expected_peer_attempts
            );
            let (transaction, rows) = query(
                begin(store, TransactionMode::Read)
                    .await
                    .expect("inspect sessions"),
                "SELECT secret_hash, csrf_hash FROM sqb_password_sessions",
                vec![],
            )
            .await
            .expect("persisted sessions");
            commit(transaction).await.expect("read complete");
            assert_eq!(rows.len(), test_case.expected_sessions);
            let stored = serde_json::to_string(&rows).expect("stored secret hashes");
            assert!(!stored.contains(&session.bearer));
            assert!(!stored.contains(&session.csrf));
            let customer = create_principal(store, context, "customers-reader")
                .await
                .expect("second person");
            set_password(
                store,
                context,
                PasswordAuthority::Operator(customer),
                password,
            )
            .await
            .expect("second password");
            let creation_fault = TransactionFault {
                description: "session audit failure",
                table: "sqb_security_audit",
                operation: "INSERT",
                condition: "WHEN NEW.action='session.created'",
                sqlite_body: "SELECT RAISE(ABORT,'injected session audit failure')",
                postgres_body: "RAISE EXCEPTION 'injected session audit failure'",
            };
            install_fault(store, &creation_fault).await;
            assert!(
                login_password(&replica, context, ("customers-reader", password))
                    .await
                    .is_err()
            );
            remove_fault(store, &creation_fault).await;
            let (transaction, rows) = query(
                begin(store, TransactionMode::Read)
                    .await
                    .expect("inspect rollback"),
                "SELECT session_id FROM sqb_password_sessions WHERE principal_id=$1",
                vec![Parameter::Id(customer)],
            )
            .await
            .expect("no unaudited session");
            commit(transaction).await.expect("read complete");
            assert!(rows.is_empty());
            let PasswordLogin::Authenticated(customer_session) =
                login_password(store, context, ("customers-reader", password))
                    .await
                    .expect("customer login")
            else {
                panic!("expected customer session");
            };
            let revocation_fault = TransactionFault {
                description: "logout audit failure",
                condition: "WHEN NEW.action='session.revoked'",
                ..creation_fault
            };
            install_fault(store, &revocation_fault).await;
            assert!(
                revoke_session(
                    begin(store, TransactionMode::Write)
                        .await
                        .expect("logout transaction"),
                    context,
                    (customer, Some(customer_session.identity.session_id))
                )
                .await
                .is_err()
            );
            remove_fault(store, &revocation_fault).await;
            assert!(
                authenticate_session(&replica, &customer_session.bearer, SessionAccess::Read)
                    .await
                    .expect("failed logout rolls back")
                    .is_some()
            );
            let proof = verify_password(store, customer, password)
                .await
                .expect("verify version")
                .expect("password proof");
            set_password(
                &replica,
                context,
                PasswordAuthority::Operator(customer),
                replacement,
            )
            .await
            .expect("operator reset");
            assert!(
                authenticate_session(store, &customer_session.bearer, SessionAccess::Read)
                    .await
                    .expect("reset invalidates session")
                    .is_none()
            );
            assert!(
                set_password(
                    store,
                    context,
                    PasswordAuthority::Reauthenticated {
                        proof,
                        session_id: customer_session.identity.session_id
                    },
                    password
                )
                .await
                .is_err()
            );
            let proof = verify_password(store, customer, replacement)
                .await
                .expect("new version")
                .expect("replacement proof");
            assert!(
                set_password(
                    store,
                    context,
                    PasswordAuthority::Reauthenticated {
                        proof,
                        session_id: customer_session.identity.session_id
                    },
                    password
                )
                .await
                .is_err()
            );
            let PasswordLogin::Authenticated(customer_session) =
                login_password(store, context, ("customers-reader", replacement))
                    .await
                    .expect("fresh login after password reset")
            else {
                panic!("expected fresh session for logout");
            };
            let (transaction, revoked) = revoke_session(
                begin(store, TransactionMode::Write)
                    .await
                    .expect("logout transaction"),
                context,
                (customer, Some(customer_session.identity.session_id)),
            )
            .await
            .expect("logout");
            assert_eq!(revoked, 1);
            commit(transaction).await.expect("publish logout");
            let (transaction, revoked) = revoke_session(
                begin(&replica, TransactionMode::Write)
                    .await
                    .expect("retry logout transaction"),
                context,
                (customer, Some(customer_session.identity.session_id)),
            )
            .await
            .expect("idempotent logout");
            assert_eq!(revoked, 0);
            commit(transaction).await.expect("finish retry");
            let PasswordLogin::Authenticated(disabled_session) =
                login_password(store, context, ("customers-reader", replacement))
                    .await
                    .expect("login before disabling person")
            else {
                panic!("expected live session before disabling person");
            };
            let transaction = begin(&replica, TransactionMode::Write)
                .await
                .expect("disable person");
            let now = transaction.now;
            let (transaction, _) = query(
                transaction,
                "UPDATE sqb_principals SET disabled_at=$1 WHERE principal_id=$2",
                vec![Parameter::Time(now), Parameter::Id(customer)],
            )
            .await
            .expect("disabled person");
            commit(transaction).await.expect("disable saved");
            assert!(
                authenticate_session(store, &disabled_session.bearer, SessionAccess::Read)
                    .await
                    .expect("disabled person's session rejected across replicas")
                    .is_none()
            );
            let transaction = begin(store, TransactionMode::Write)
                .await
                .expect("expire session");
            let expired = transaction.now - Duration::seconds(1);
            let (transaction, _) = query(
                transaction,
                "UPDATE sqb_password_sessions SET expires_at=$1 WHERE session_id=$2",
                vec![
                    Parameter::Time(expired),
                    Parameter::Id(session.identity.session_id),
                ],
            )
            .await
            .expect("expired session");
            commit(transaction).await.expect("expiry saved");
            assert!(
                authenticate_session(&replica, &session.bearer, SessionAccess::Read)
                    .await
                    .expect("expired session rejected")
                    .is_none()
            );
            let (transaction, rows) = query(
                begin(store, TransactionMode::Read)
                    .await
                    .expect("session history"),
                "SELECT revoked_at FROM sqb_password_sessions WHERE session_id=$1",
                vec![Parameter::Id(customer_session.identity.session_id)],
            )
            .await
            .expect("revoked history retained");
            commit(transaction).await.expect("history complete");
            assert!(!rows[0]["revoked_at"].is_null());
            let (transaction, rows) = query(
                begin(store, TransactionMode::Read)
                    .await
                    .expect("audit history"),
                "SELECT action, after_json FROM sqb_security_audit WHERE event_id=$1",
                vec![Parameter::Id(Uuid::new_v5(
                    &customer_session.identity.session_id,
                    b"revoked",
                ))],
            )
            .await
            .expect("one revocation audit");
            commit(transaction).await.expect("audit complete");
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0]["action"], json!("session.revoked"));
            close_store(replica).await;
            close_test_store(fixture).await;
        });
    }
}
