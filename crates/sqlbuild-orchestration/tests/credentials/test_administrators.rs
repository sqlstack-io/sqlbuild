//! Installation bootstrap and final-admin transitions serialize across real database replicas.

use futures_util::future::join_all;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::authorization::main::{
    bootstrap_user::bootstrap_user, change_administrator::change_administrator,
};
use crate::credentials::main::{
    identities::create_principal::create_principal, sessions::login_password::login_password,
    sessions::revoke_session::revoke_session, sessions::set_password::set_password,
};
use crate::credentials::models::{
    AdministratorAuthentication, AdministratorChange, AuditContext, PasswordAuthority,
    PasswordLogin,
};
use crate::credentials::tests::helpers::security_event_count;
use crate::credentials::tests::test_types::AdministratorTestCase;
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
fn given_installation_administrators_when_replicas_race_then_bootstrap_is_once_only_and_final_admin_survives()
 {
    let test_cases = [
        AdministratorTestCase {
            description: "SQLite once-only bootstrap and final admin",
            backend: Backend::Sqlite,
            expected_administrators: 1,
        },
        AdministratorTestCase {
            description: "PostgreSQL once-only bootstrap and final admin",
            backend: Backend::Postgres,
            expected_administrators: 1,
        },
    ];
    for test_case in test_cases {
        for change in [AdministratorChange::Revoke, AdministratorChange::Disable] {
            run(async {
                let fixture = open_test_store(test_case.backend).await.expect("database");
                let store = &fixture.store;
                migrate_store(store).await.expect("migrate");
                let replica = second_store(store).await;
                let context = AuditContext {
                    actor: "local-control-store-owner".into(),
                    granting_role: "control-store-owner".into(),
                    request_id: Uuid::now_v7(),
                    ip_address: Some("127.0.0.1".parse().expect("trusted peer")),
                };
                let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../benchmarks/duckshop/edge_cases/orchestration/administrator_lifecycle.json");
                let inputs: Value = serde_json::from_str(
                    &std::fs::read_to_string(path).expect("DuckShop lifecycle"),
                )
                .expect("inputs");
                let names = [
                    inputs["first_person"].as_str().expect("first person"),
                    inputs["second_person"].as_str().expect("second person"),
                ];
                let stores = [store, &replica];
                let results = join_all(
                    (0..2).map(|index| bootstrap_user(stores[index], &context, names[index])),
                )
                .await;
                assert_eq!(
                    results.iter().filter(|result| result.is_ok()).count(),
                    test_case.expected_administrators,
                    "{}",
                    test_case.description
                );
                let (winner, first) = results
                    .into_iter()
                    .enumerate()
                    .find_map(|(index, result)| result.ok().map(|id| (index, id)))
                    .expect("one bootstrap winner");
                let other = 1 - winner;
                let second = create_principal(store, &context, names[other])
                    .await
                    .expect("ordinary role-free person");
                let password = inputs["password"].as_str().expect("password input");
                let mut sessions = Vec::new();
                for (id, name) in [(first, names[winner]), (second, names[other])] {
                    set_password(store, &context, PasswordAuthority::Operator(id), password)
                        .await
                        .expect("owner provisioning");
                    let PasswordLogin::Authenticated(session) =
                        login_password(store, &context, (name, password))
                            .await
                            .expect("login")
                    else {
                        panic!("valid login");
                    };
                    sessions.push(session);
                }
                let first_credentials = (sessions[0].bearer.as_str(), sessions[0].csrf.as_str());
                let second_credentials = (sessions[1].bearer.as_str(), sessions[1].csrf.as_str());
                assert!(
                    change_administrator(
                        store,
                        &context,
                        second_credentials,
                        (
                            second,
                            AdministratorChange::Grant,
                            &AdministratorAuthentication::Password
                        ),
                    )
                    .await
                    .is_err(),
                    "a valid person session is not administrator authority"
                );
                assert!(
                    change_administrator(
                        store,
                        &context,
                        (first_credentials.0, "wrong-csrf"),
                        (
                            second,
                            AdministratorChange::Grant,
                            &AdministratorAuthentication::Password
                        ),
                    )
                    .await
                    .is_err()
                );
                assert!(
                    change_administrator(
                        store,
                        &context,
                        first_credentials,
                        (first, change, &AdministratorAuthentication::Password)
                    )
                    .await
                    .is_err(),
                    "cannot remove sole administrator"
                );
                assert!(
                    change_administrator(
                        store,
                        &context,
                        first_credentials,
                        (
                            second,
                            AdministratorChange::Grant,
                            &AdministratorAuthentication::Password
                        ),
                    )
                    .await
                    .expect("authenticated administrator grant")
                );
                let revision = security_event_count(store)
                    .await
                    .expect("revision before duplicate grant");
                assert!(
                    !change_administrator(
                        store,
                        &context,
                        first_credentials,
                        (
                            second,
                            AdministratorChange::Grant,
                            &AdministratorAuthentication::Password
                        ),
                    )
                    .await
                    .expect("idempotent grant")
                );
                assert_eq!(
                    security_event_count(store)
                        .await
                        .expect("revision after duplicate grant"),
                    revision
                );
                let (left, right) = tokio::join!(
                    change_administrator(
                        store,
                        &context,
                        first_credentials,
                        (first, change, &AdministratorAuthentication::Password)
                    ),
                    change_administrator(
                        &replica,
                        &context,
                        second_credentials,
                        (second, change, &AdministratorAuthentication::Password)
                    )
                );
                assert_eq!(
                    [&left, &right]
                        .iter()
                        .filter(|result| matches!(result, Ok(true)))
                        .count(),
                    1,
                    "concurrent self-removal leaves one administrator"
                );
                let survivor = usize::from(left.is_ok());
                let survivor_credentials = (
                    sessions[survivor].bearer.as_str(),
                    sessions[survivor].csrf.as_str(),
                );
                let (transaction, rows) = query(begin(store, TransactionMode::Read).await.expect("inspect remaining admins"),
                    "SELECT COUNT(*) AS count FROM sqb_role_assignments a JOIN sqb_principals p USING(principal_id) WHERE a.role_key='admin' AND a.revoked_at IS NULL AND p.disabled_at IS NULL",vec![]).await.expect("count remaining admins");
                assert_eq!(rows[0]["count"], json!(1));
                commit(transaction)
                    .await
                    .expect("finish remaining-admin read");
                let (transaction, audit) = query(begin(store,TransactionMode::Read).await.expect("read grant audit"),
                    "SELECT actor,granting_role FROM sqb_security_audit WHERE action='role.assigned' AND actor<> 'local-control-store-owner'",vec![]).await.expect("grant audit");
                assert_eq!(audit.len(), 1);
                assert_eq!(audit[0]["actor"], json!(format!("principal:{first}")));
                assert_eq!(audit[0]["granting_role"], json!("admin"));
                commit(transaction).await.expect("finish audit read");
                let identity = &sessions[survivor].identity;
                let (transaction, _) = revoke_session(
                    begin(store, TransactionMode::Write)
                        .await
                        .expect("logout transaction"),
                    &context,
                    (identity.principal_id, Some(identity.session_id)),
                )
                .await
                .expect("logout");
                commit(transaction).await.expect("publish logout");
                assert!(
                    change_administrator(
                        &replica,
                        &context,
                        survivor_credentials,
                        (
                            first,
                            AdministratorChange::Grant,
                            &AdministratorAuthentication::Password
                        ),
                    )
                    .await
                    .is_err(),
                    "revoked session cannot grant"
                );
                // Simulate an operator's out-of-band loss: it must never reopen bootstrap.
                let (transaction, _) = query(
                    begin(store, TransactionMode::Write)
                        .await
                        .expect("simulate out-of-band loss"),
                    "UPDATE sqb_principals SET disabled_at=$1",
                    vec![Parameter::Time(chrono::Utc::now())],
                )
                .await
                .expect("disable all people outside supported API");
                commit(transaction).await.expect("commit simulated loss");
                assert!(
                    bootstrap_user(&replica, &context, "replacement-admin")
                        .await
                        .is_err()
                );
                close_store(replica).await;
                close_test_store(fixture).await;
            });
        }
    }
}

#[test]
fn given_bootstrap_and_admin_changes_when_durable_publication_fails_then_all_authority_rolls_back()
{
    let test_cases = [
        AdministratorTestCase {
            description: "SQLite publication failures roll back authority",
            backend: Backend::Sqlite,
            expected_administrators: 0,
        },
        AdministratorTestCase {
            description: "PostgreSQL publication failures roll back authority",
            backend: Backend::Postgres,
            expected_administrators: 0,
        },
    ];
    for test_case in test_cases {
        {
            let (table, operation) = ("sqb_security_audit", "INSERT");
            run(async {
                let fixture = open_test_store(test_case.backend).await.expect("database");
                let store = &fixture.store;
                migrate_store(store).await.expect("migrate");
                let context = AuditContext {
                    actor: "owner".into(),
                    granting_role: "control-store-owner".into(),
                    request_id: Uuid::now_v7(),
                    ip_address: Some("127.0.0.1".parse().expect("trusted peer")),
                };
                let fault = TransactionFault {
                    description: "fail administrator publication",
                    table,
                    operation,
                    condition: "",
                    sqlite_body: "SELECT RAISE(ABORT, 'publication unavailable')",
                    postgres_body: "RAISE EXCEPTION 'publication unavailable'",
                };
                install_fault(store, &fault).await;
                assert!(
                    bootstrap_user(store, &context, "orders-admin")
                        .await
                        .is_err()
                );
                remove_fault(store, &fault).await;
                let (transaction,rows) = query(begin(store,TransactionMode::Read).await.expect("inspect rollback"),"SELECT (SELECT COUNT(*) FROM sqb_principals) AS people, (SELECT COUNT(*) FROM sqb_admin_initialization) AS initialized, (SELECT COUNT(*) FROM sqb_role_assignments) AS assignments",vec![]).await.expect("count rolled back bootstrap");
                assert_eq!(
                    serde_json::to_value(&rows[0]).expect("serializable row"),
                    json!({"people":0,"initialized":0,"assignments":0})
                );
                assert_eq!(
                    rows[0]["assignments"],
                    json!(test_case.expected_administrators),
                    "{}",
                    test_case.description
                );
                commit(transaction).await.expect("finish rollback read");
                assert_eq!(
                    security_event_count(store)
                        .await
                        .expect("rolled back revision"),
                    0
                );
                let first = bootstrap_user(store, &context, "orders-admin")
                    .await
                    .expect("retry after rollback");
                let second = create_principal(store, &context, "customers-admin")
                    .await
                    .expect("second person");
                set_password(
                    store,
                    &context,
                    PasswordAuthority::Operator(first),
                    "orders-password-2026",
                )
                .await
                .expect("owner password provisioning");
                let PasswordLogin::Authenticated(session) =
                    login_password(store, &context, ("orders-admin", "orders-password-2026"))
                        .await
                        .expect("password login")
                else {
                    panic!("login");
                };
                let credentials = (session.bearer.as_str(), session.csrf.as_str());
                let revision = security_event_count(store)
                    .await
                    .expect("revision before failed grant");
                install_fault(store, &fault).await;
                assert!(
                    change_administrator(
                        store,
                        &context,
                        credentials,
                        (
                            second,
                            AdministratorChange::Grant,
                            &AdministratorAuthentication::Password
                        ),
                    )
                    .await
                    .is_err()
                );
                remove_fault(store, &fault).await;
                assert_eq!(
                    security_event_count(store)
                        .await
                        .expect("revision after failed grant"),
                    revision
                );
                assert!(
                    change_administrator(
                        store,
                        &context,
                        credentials,
                        (
                            second,
                            AdministratorChange::Grant,
                            &AdministratorAuthentication::Password
                        ),
                    )
                    .await
                    .expect("retry grant"),
                    "failed grant did not persist"
                );
                assert!(
                    change_administrator(
                        store,
                        &context,
                        credentials,
                        (
                            first,
                            AdministratorChange::Disable,
                            &AdministratorAuthentication::Password
                        ),
                    )
                    .await
                    .is_err(),
                    "an unprovisioned administrator is not a usable password-authentication alternative"
                );
                set_password(
                    store,
                    &context,
                    PasswordAuthority::Operator(second),
                    "customers-password-2026",
                )
                .await
                .expect("provision alternative administrator");
                install_fault(store, &fault).await;
                assert!(
                    change_administrator(
                        store,
                        &context,
                        credentials,
                        (
                            first,
                            AdministratorChange::Disable,
                            &AdministratorAuthentication::Password
                        ),
                    )
                    .await
                    .is_err()
                );
                remove_fault(store, &fault).await;
                assert!(
                    change_administrator(
                        store,
                        &context,
                        credentials,
                        (
                            first,
                            AdministratorChange::Disable,
                            &AdministratorAuthentication::Password
                        ),
                    )
                    .await
                    .expect("retry disabling person"),
                    "failed disable preserved live session and enabled person"
                );
                close_test_store(fixture).await;
            });
        }
    }
}
