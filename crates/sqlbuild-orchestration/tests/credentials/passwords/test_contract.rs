//! Real-store password replacement, live eligibility and transactional security audit.

use serde_json::{Value, json};
use uuid::Uuid;

use crate::credentials::main::{
    identities::create_principal::create_principal, sessions::set_password::set_password,
    sessions::verify_password::verify_password,
};
use crate::credentials::models::{AuditContext, PasswordAuthority};
use crate::credentials::tests::helpers::security_event_count;
use crate::store::main::{
    begin::begin, commit::commit, migrate_store::migrate_store, query::query,
};
use crate::store::models::{Parameter, TransactionMode};
use crate::store::tests::fault_helpers::{install_fault, remove_fault};
use crate::store::tests::fault_types::TransactionFault;
use crate::store::tests::replica_helpers::second_store;
use crate::store::tests::store_helpers::{close_store, close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

use crate::credentials::tests::test_passwords::test_types::PasswordTestCase;

#[path = "test_types.rs"]
mod test_types;

#[test]
fn given_person_passwords_when_changed_then_replicas_verify_exact_bytes_and_audit_failure_rolls_back()
 {
    let test_cases = [
        PasswordTestCase {
            description: "SQLite password lifecycle",
            backend: Backend::Sqlite,
            expected_audits: 5,
        },
        PasswordTestCase {
            description: "PostgreSQL password lifecycle",
            backend: Backend::Postgres,
            expected_audits: 5,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("fresh schema");
            let replica = second_store(store).await;
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
                "../../benchmarks/duckshop/edge_cases/orchestration/password_credentials.json",
            );
            let cases: Value =
                serde_json::from_str(&std::fs::read_to_string(path).expect("DuckShop fixture"))
                    .expect("password cases");
            let password = cases["password"].as_str().expect("password");
            let replacement = cases["replacement"].as_str().expect("replacement");
            let context = AuditContext {
                actor: "test-admin".into(),
                granting_role: "admin".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            let person = create_principal(store, &context, "orders-reader")
                .await
                .expect("person");
            for id in [person, Uuid::now_v7()] {
                assert!(
                    verify_password(&replica, id, password)
                        .await
                        .expect("missing credential")
                        .is_none()
                );
            }
            for weak in [
                "",
                "12345678901",
                cases["too_short"].as_str().expect("short Unicode"),
            ] {
                assert!(
                    set_password(store, &context, PasswordAuthority::Operator(person), weak)
                        .await
                        .is_err()
                );
            }
            assert!(
                set_password(
                    store,
                    &context,
                    PasswordAuthority::Operator(person),
                    &"x".repeat(1025)
                )
                .await
                .is_err()
            );
            set_password(
                store,
                &context,
                PasswordAuthority::Operator(person),
                password,
            )
            .await
            .expect("set password");
            let first = verify_password(&replica, person, password)
                .await
                .expect("verify on second store")
                .expect("password proof");
            assert_eq!(first.principal_id, person, "{}", test_case.description);
            let transaction = begin(&replica, TransactionMode::Read)
                .await
                .expect("inspect storage");
            let (transaction, rows) = query(
                transaction,
                "SELECT password_hash FROM sqb_password_credentials WHERE principal_id=$1",
                vec![Parameter::Id(person)],
            )
            .await
            .expect("stored PHC");
            commit(transaction).await.expect("inspection done");
            let first_hash = rows[0]["password_hash"].as_str().expect("PHC").to_owned();
            assert!(first_hash.starts_with("$argon2id$v=19$m=19456,t=2,p=1$"));
            assert!(!first_hash.contains(password));
            set_password(
                store,
                &context,
                PasswordAuthority::Operator(person),
                password,
            )
            .await
            .expect("same bytes, new salt");
            let second = verify_password(&replica, person, password)
                .await
                .expect("new version")
                .expect("valid password");
            assert_ne!(first.credential_id, second.credential_id);
            let (transaction, rows) = query(
                begin(&replica, TransactionMode::Read).await.expect("read"),
                "SELECT password_hash FROM sqb_password_credentials WHERE principal_id=$1",
                vec![Parameter::Id(person)],
            )
            .await
            .expect("replacement salt");
            commit(transaction).await.expect("read done");
            assert_ne!(
                rows[0]["password_hash"],
                json!(first_hash),
                "identical passwords receive independent salts"
            );
            let fault = TransactionFault {
                description: "password audit failure",
                table: "sqb_security_audit",
                operation: "INSERT",
                condition: "",
                sqlite_body: "SELECT RAISE(ABORT,'injected audit failure')",
                postgres_body: "RAISE EXCEPTION 'injected audit failure'",
            };
            let revision = security_event_count(&replica)
                .await
                .expect("password revision");
            install_fault(store, &fault).await;
            assert!(
                set_password(
                    store,
                    &context,
                    PasswordAuthority::Operator(person),
                    replacement
                )
                .await
                .is_err()
            );
            remove_fault(store, &fault).await;
            assert_eq!(
                security_event_count(&replica)
                    .await
                    .expect("audit rollback revision"),
                revision
            );
            assert_eq!(
                verify_password(&replica, person, password)
                    .await
                    .expect("rolled back"),
                Some(second)
            );
            set_password(
                store,
                &context,
                PasswordAuthority::Operator(person),
                replacement,
            )
            .await
            .expect("replace password");
            assert_eq!(
                security_event_count(&replica)
                    .await
                    .expect("replacement revision"),
                revision + 1
            );
            for invalid in [
                password,
                cases["wrong"].as_str().expect("wrong"),
                &"x".repeat(1025),
            ] {
                assert!(
                    verify_password(&replica, person, invalid)
                        .await
                        .expect("rejected password")
                        .is_none()
                );
            }
            assert!(
                verify_password(&replica, person, replacement)
                    .await
                    .expect("new password")
                    .is_some()
            );
            for boundary in [
                cases["minimum_unicode"]
                    .as_str()
                    .expect("12 Unicode characters"),
                &"x".repeat(1024),
            ] {
                set_password(
                    store,
                    &context,
                    PasswordAuthority::Operator(person),
                    boundary,
                )
                .await
                .expect("accepted boundary");
                assert!(
                    verify_password(&replica, person, boundary)
                        .await
                        .expect("boundary verified")
                        .is_some()
                );
            }
            migrate_store(&replica)
                .await
                .expect("restart preserves credentials");
            let (transaction, rows) = query(begin(&replica, TransactionMode::Read).await.expect("audit read"),
                "SELECT action, before_json, after_json FROM sqb_security_audit WHERE action='password.changed'", vec![])
                .await.expect("durable audits");
            commit(transaction).await.expect("audit read done");
            assert_eq!(
                rows.len(),
                test_case.expected_audits,
                "failed and rejected changes leave no audit or credential change"
            );
            let audit = serde_json::to_string(&rows).expect("audit inspection");
            for secret in [password, replacement, &first_hash] {
                assert!(
                    !audit.contains(secret),
                    "audit contains identities, never passwords or hashes"
                );
            }
            let (transaction, _) = query(
                begin(store, TransactionMode::Write).await.expect("disable"),
                "UPDATE sqb_principals SET disabled_at=$1 WHERE principal_id=$2",
                vec![Parameter::Time(chrono::Utc::now()), Parameter::Id(person)],
            )
            .await
            .expect("disabled person");
            commit(transaction).await.expect("disable committed");
            assert!(
                verify_password(&replica, person, &"x".repeat(1024))
                    .await
                    .expect("live eligibility")
                    .is_none()
            );
            assert!(
                set_password(
                    store,
                    &context,
                    PasswordAuthority::Operator(person),
                    replacement
                )
                .await
                .is_err()
            );
            close_store(replica).await;
            close_test_store(fixture).await;
        });
    }
}
