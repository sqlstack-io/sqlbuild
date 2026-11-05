//! Recovery is exclusive, atomic, audited, and cannot reopen first-admin initialization.

use std::time::Duration;

use serde_json::{Value, json};
use uuid::Uuid;

use crate::authorization::main::{
    bootstrap_user::bootstrap_user, recover_administrator::recover_administrator,
};
use crate::credentials::main::{
    identities::create_principal::create_principal,
    sessions::authenticate_bound_person::authenticate_bound_person,
    sessions::authenticate_session::authenticate_session, sessions::login_password::login_password,
    sessions::set_password::set_password,
};
use crate::credentials::models::{
    AuditContext, ExternalIdentityMechanism, PasswordAuthority, PasswordLogin, SessionAccess,
};
use crate::credentials::tests::helpers::security_event_count;
use crate::credentials::tests::test_types::AdministratorRecoveryTestCase;
use crate::store::main::{
    begin::begin, begin_offline_recovery::begin_offline_recovery, commit::commit,
    migrate_store::migrate_store, open_store::open_store, query::query,
};
use crate::store::models::{ControlStore, Parameter, TransactionMode};
use crate::store::tests::fault_helpers::{install_fault, remove_fault};
use crate::store::tests::fault_types::TransactionFault;
use crate::store::tests::replica_helpers::store_url;
use crate::store::tests::store_helpers::{close_store, close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_lost_administrator_access_when_offline_recovery_runs_then_authority_and_audit_commit_atomically()
 {
    let test_cases = [
        AdministratorRecoveryTestCase {
            description: "SQLite exclusive audited recovery",
            backend: Backend::Sqlite,
            expected_recoveries: 1,
        },
        AdministratorRecoveryTestCase {
            description: "PostgreSQL exclusive audited recovery",
            backend: Backend::Postgres,
            expected_recoveries: 1,
        },
    ];
    for test_case in test_cases {
        run(async {
            let mut fixture = open_test_store(test_case.backend).await.expect("database");
            migrate_store(&fixture.store).await.expect("migrate");
            let url = store_url(&fixture.store);
            let context = AuditContext {
                actor: "local-control-store-owner".into(),
                granting_role: "control-store-owner".into(),
                request_id: Uuid::now_v7(),
                ip_address: Some("127.0.0.1".parse().expect("trusted local peer")),
            };
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
                "../../benchmarks/duckshop/edge_cases/orchestration/administrator_lifecycle.json",
            );
            let inputs: Value =
                serde_json::from_str(&std::fs::read_to_string(path).expect("DuckShop recovery"))
                    .expect("inputs");
            let reason = inputs["recovery_reason"].as_str().expect("recovery reason");
            let name = inputs["first_person"].as_str().expect("person name");
            let password = inputs["password"].as_str().expect("password");
            let ordinary = create_principal(&fixture.store, &context, "customers-reader")
                .await
                .expect("ordinary person");
            close_store(fixture.store).await;
            assert!(
                recover_administrator(
                    begin_offline_recovery(&url)
                        .await
                        .expect("offline empty installation"),
                    &context,
                    (ordinary, reason)
                )
                .await
                .is_err(),
                "recovery cannot initialize an admin"
            );
            fixture.store = open_store(&url).await.expect("reopen before bootstrap");
            let principal = bootstrap_user(&fixture.store, &context, name)
                .await
                .expect("first admin");
            set_password(
                &fixture.store,
                &context,
                PasswordAuthority::Operator(principal),
                password,
            )
            .await
            .expect("password");
            let PasswordLogin::Authenticated(session) =
                login_password(&fixture.store, &context, (name, password))
                    .await
                    .expect("login")
            else {
                panic!("valid password");
            };
            let external_session = authenticate_bound_person(
                &fixture.store,
                &context,
                principal,
                ExternalIdentityMechanism::Oidc,
            )
            .await
            .expect("verified OIDC identity")
            .expect("SSO session");
            assert!(
                begin_offline_recovery(&url).await.is_err(),
                "even idle normal clients exclude recovery"
            );
            let (transaction, _) = query(
                begin(&fixture.store, TransactionMode::Write)
                    .await
                    .expect("simulate loss"),
                "UPDATE sqb_principals SET disabled_at=$1 WHERE principal_id=$2",
                vec![
                    Parameter::Time(chrono::Utc::now()),
                    Parameter::Id(principal),
                ],
            )
            .await
            .expect("disable administrator");
            let now = transaction.now;
            let (transaction, _) = query(transaction, "UPDATE sqb_role_assignments SET revoked_at=$1,active_key=NULL WHERE principal_id=$2", vec![Parameter::Time(now), Parameter::Id(principal)]).await.expect("lose administrator assignment");
            commit(transaction).await.expect("commit simulated loss");
            let revision = security_event_count(&fixture.store)
                .await
                .expect("revision");
            for (table, operation) in [
                ("sqb_security_audit", "INSERT"),
                ("sqb_role_assignments", "INSERT"),
                ("sqb_password_sessions", "UPDATE"),
                ("sqb_external_sessions", "UPDATE"),
            ] {
                let fault = TransactionFault {
                    description: "recovery publication fails",
                    table,
                    operation,
                    condition: "",
                    sqlite_body: "SELECT RAISE(ABORT, 'recovery unavailable')",
                    postgres_body: "RAISE EXCEPTION 'recovery unavailable'",
                };
                install_fault(&fixture.store, &fault).await;
                close_store(fixture.store).await;
                let recovery = begin_offline_recovery(&url)
                    .await
                    .expect("exclusive access after stop");
                assert!(
                    recover_administrator(recovery, &context, (principal, reason))
                        .await
                        .is_err()
                );
                fixture.store = open_store(&url)
                    .await
                    .expect("reopen after failed recovery");
                remove_fault(&fixture.store, &fault).await;
                assert_eq!(
                    security_event_count(&fixture.store)
                        .await
                        .expect("revision after rollback"),
                    revision
                );
                let (transaction, rows) = query(begin(&fixture.store, TransactionMode::Read).await.expect("inspect rollback"),
                    "SELECT (SELECT COUNT(*) FROM sqb_principals WHERE disabled_at IS NOT NULL) AS disabled, (SELECT COUNT(*) FROM sqb_role_assignments WHERE revoked_at IS NULL) AS roles, (SELECT COUNT(*) FROM sqb_password_sessions WHERE revoked_at IS NULL) AS sessions, (SELECT COUNT(*) FROM sqb_external_sessions WHERE revoked_at IS NULL) AS external_sessions, (SELECT COUNT(*) FROM sqb_security_audit WHERE action='admin.offline_recovered') AS recovered", vec![]).await.expect("rollback counts");
                assert_eq!(
                    serde_json::to_value(&rows[0]).expect("row"),
                    json!({"disabled":1,"roles":0,"sessions":1,"external_sessions":1,"recovered":0})
                );
                commit(transaction).await.expect("finish rollback read");
            }
            close_store(fixture.store).await;
            for (id, explanation) in [(principal, " "), (Uuid::nil(), reason)] {
                assert!(
                    recover_administrator(
                        begin_offline_recovery(&url)
                            .await
                            .expect("offline invalid request"),
                        &context,
                        (id, explanation)
                    )
                    .await
                    .is_err()
                );
            }
            let recovery = begin_offline_recovery(&url)
                .await
                .expect("offline recovery capability");
            assert!(
                !matches!(
                    tokio::time::timeout(Duration::from_millis(100), open_store(&url)).await,
                    Ok(Ok(_))
                ),
                "normal clients cannot enter recovery"
            );
            recover_administrator(recovery, &context, (principal, reason))
                .await
                .expect("restore administrator");
            fixture.store = open_store(&url)
                .await
                .expect("reopen recovered installation");
            assert!(
                authenticate_session(&fixture.store, &session.bearer, SessionAccess::Read)
                    .await
                    .expect("old session")
                    .is_none()
            );
            assert!(
                authenticate_session(
                    &fixture.store,
                    &external_session.bearer,
                    SessionAccess::Read
                )
                .await
                .expect("old SSO session")
                .is_none()
            );
            assert!(matches!(
                login_password(&fixture.store, &context, (name, password))
                    .await
                    .expect("fresh password login"),
                PasswordLogin::Authenticated(_)
            ));
            assert!(
                bootstrap_user(&fixture.store, &context, "replacement-admin")
                    .await
                    .is_err(),
                "recovery never reopens bootstrap"
            );
            let (transaction, rows) = query(begin(&fixture.store, TransactionMode::Read).await.expect("inspect recovery"),
                "SELECT actor, granting_role, after_json FROM sqb_security_audit WHERE action='admin.offline_recovered'", vec![]).await.expect("durable recovery audit");
            assert_eq!(
                rows.len(),
                test_case.expected_recoveries,
                "{}",
                test_case.description
            );
            assert_eq!(rows[0]["actor"], "local-control-store-owner");
            assert_eq!(rows[0]["granting_role"], "control-store-owner");
            let after = &rows[0]["after_json"];
            assert_eq!(after["reason"], reason);
            assert_eq!(after["sessions_revoked"], 2);
            commit(transaction).await.expect("finish audit read");
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_a_cancelled_sqlite_query_when_blocking_work_continues_then_exclusive_access_stays_locked_until_rollback()
 {
    let test_cases = [AdministratorRecoveryTestCase {
        description: "SQLite cancellation cannot release exclusive access early",
        backend: Backend::Sqlite,
        expected_recoveries: 0,
    }];
    for test_case in test_cases {
        run(async {
            let mut fixture = open_test_store(test_case.backend).await.expect("database");
            migrate_store(&fixture.store).await.expect("migrate");
            let url = store_url(&fixture.store);
            close_store(fixture.store).await;
            let crate::store::models::OfflineRecovery(mut transaction) =
                begin_offline_recovery(&url)
                    .await
                    .expect("exclusive transaction");
            let crate::store::models::Connection::Sqlite(connection, _) =
                transaction.connection.as_mut().expect("owned connection")
            else {
                unreachable!()
            };
            let (started, observed) = std::sync::mpsc::channel();
            let (release, wait) = std::sync::mpsc::channel();
            connection
                .create_scalar_function(
                    "wait_for_release",
                    0,
                    rusqlite::functions::FunctionFlags::SQLITE_UTF8,
                    move |_| {
                        started.send(()).expect("notify query start");
                        wait.recv().expect("release blocking query");
                        Ok(1_i64)
                    },
                )
                .expect("blocking database function");
            let work = tokio::spawn(query(transaction, "SELECT wait_for_release()", vec![]));
            tokio::task::spawn_blocking(move || {
                observed
                    .recv_timeout(Duration::from_secs(5))
                    .expect("query started")
            })
            .await
            .expect("observe SQLite query");
            work.abort();
            assert!(work.await.expect_err("query task cancelled").is_cancelled());
            assert!(
                open_store(&url).await.is_err(),
                "normal readers cannot enter an unfinished offline transaction"
            );
            assert!(
                begin_offline_recovery(&url).await.is_err(),
                "blocking SQLite work retains the access guard after cancellation"
            );
            release.send(()).expect("finish query");
            tokio::time::timeout(Duration::from_secs(5), async {
                for _ in 0..500 {
                    let Ok(recovery) = begin_offline_recovery(&url).await else {
                        tokio::time::sleep(Duration::from_millis(10)).await;
                        continue;
                    };
                    drop(recovery);
                    return;
                }
                panic!("exclusive access never resumed");
            })
            .await
            .expect("exclusive access after rollback");
            fixture.store = open_store(&url).await.expect("normal access resumes");
            assert!(matches!(fixture.store, ControlStore::Sqlite(_, _)));
            let (transaction, rows) = query(
                begin(&fixture.store, TransactionMode::Read)
                    .await
                    .expect("inspect abandoned recovery"),
                "SELECT event_id FROM sqb_security_audit WHERE action='admin.offline_recovered'",
                vec![],
            )
            .await
            .expect("no successful recovery audit");
            assert_eq!(
                rows.len(),
                test_case.expected_recoveries,
                "{}",
                test_case.description
            );
            commit(transaction)
                .await
                .expect("finish abandoned recovery read");
            close_test_store(fixture).await;
        });
    }
}
