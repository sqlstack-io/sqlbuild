//! grant-admin works online, is atomic and audited, and cannot reopen first-admin initialization.

use serde_json::{Value, json};
use uuid::Uuid;

use crate::authorization::main::{bootstrap_user::bootstrap_user, grant_admin::grant_admin};
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
    begin::begin, commit::commit, migrate_store::migrate_store, query::query,
};
use crate::store::models::{Parameter, TransactionMode};
use crate::store::tests::fault_helpers::{install_fault, remove_fault};
use crate::store::tests::fault_types::TransactionFault;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_lost_administrator_access_when_grant_admin_runs_online_then_authority_and_audit_commit_atomically()
 {
    let test_cases = [
        AdministratorRecoveryTestCase {
            description: "SQLite audited grant-admin",
            backend: Backend::Sqlite,
            expected_recoveries: 1,
        },
        AdministratorRecoveryTestCase {
            description: "PostgreSQL audited grant-admin",
            backend: Backend::Postgres,
            expected_recoveries: 1,
        },
    ];
    for test_case in test_cases {
        run(async {
            let fixture = open_test_store(test_case.backend).await.expect("database");
            migrate_store(&fixture.store).await.expect("migrate");
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
            let name = inputs["first_person"].as_str().expect("person name");
            let password = inputs["password"].as_str().expect("password");
            let ordinary = create_principal(&fixture.store, &context, "customers-reader")
                .await
                .expect("ordinary person");
            assert!(
                grant_admin(&fixture.store, &context, ordinary)
                    .await
                    .is_err(),
                "grant-admin cannot initialize an admin"
            );
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
                ("sqb_sessions", "UPDATE"),
            ] {
                let fault = TransactionFault {
                    description: "grant-admin publication fails",
                    table,
                    operation,
                    condition: "",
                    sqlite_body: "SELECT RAISE(ABORT, 'recovery unavailable')",
                    postgres_body: "RAISE EXCEPTION 'recovery unavailable'",
                };
                install_fault(&fixture.store, &fault).await;
                assert!(
                    grant_admin(&fixture.store, &context, principal)
                        .await
                        .is_err()
                );
                remove_fault(&fixture.store, &fault).await;
                assert_eq!(
                    security_event_count(&fixture.store)
                        .await
                        .expect("revision after rollback"),
                    revision
                );
                let (transaction, rows) = query(begin(&fixture.store, TransactionMode::Read).await.expect("inspect rollback"),
                    "SELECT (SELECT COUNT(*) FROM sqb_principals WHERE disabled_at IS NOT NULL) AS disabled, (SELECT COUNT(*) FROM sqb_role_assignments WHERE revoked_at IS NULL) AS roles, (SELECT COUNT(*) FROM sqb_sessions WHERE revoked_at IS NULL) AS sessions, (SELECT COUNT(*) FROM sqb_security_audit WHERE action='admin.granted') AS recovered", vec![]).await.expect("rollback counts");
                assert_eq!(
                    serde_json::to_value(&rows[0]).expect("row"),
                    json!({"disabled":1,"roles":0,"sessions":2,"recovered":0})
                );
                commit(transaction).await.expect("finish rollback read");
            }
            assert!(
                grant_admin(&fixture.store, &context, Uuid::nil())
                    .await
                    .is_err(),
                "grant-admin requires an existing person"
            );
            grant_admin(&fixture.store, &context, principal)
                .await
                .expect("restore administrator while the store stays open");
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
                "grant-admin never reopens bootstrap"
            );
            let (transaction, rows) = query(begin(&fixture.store, TransactionMode::Read).await.expect("inspect recovery"),
                "SELECT actor, granting_role, after_json FROM sqb_security_audit WHERE action='admin.granted'", vec![]).await.expect("durable recovery audit");
            assert_eq!(
                rows.len(),
                test_case.expected_recoveries,
                "{}",
                test_case.description
            );
            assert_eq!(rows[0]["actor"], "local-control-store-owner");
            assert_eq!(rows[0]["granting_role"], "control-store-owner");
            let after = &rows[0]["after_json"];
            assert_eq!(after["sessions_revoked"], 2);
            commit(transaction).await.expect("finish audit read");
            close_test_store(fixture).await;
        });
    }
}
