use std::collections::BTreeMap;

use crate::credentials::main::authenticate_token::authenticate_token;
use crate::credentials::main::create_location::create_location;
use crate::credentials::main::create_principal::create_principal;
use crate::credentials::main::issue_token::issue_token;
use crate::credentials::main::revoke_token::revoke_token;
use crate::credentials::main::rotate_token::rotate_token;
use crate::credentials::models::{AuditContext, CredentialScope, PrincipalKind};
use crate::runs::main::claim_next_run::claim_next_run;
use crate::scheduler::tests::contracts::helpers::acquire_lease;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::store::main::migrate_store::migrate_store;
use chrono::{Duration, Utc};
use uuid::Uuid;

use crate::credentials::tests::helpers::{authentication_revision, location_state, stored_hashes};
use crate::credentials::tests::test_types::{CredentialTestCase, TransactionFault};
use crate::runs::tests::helpers::{queue, request, snapshot};
use crate::runs::tests::terminal_helpers::finish;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::TransactionMode;
use crate::store::tests::fault_helpers::{install_fault, remove_fault};
use crate::store::tests::replica_helpers::second_store;
use crate::store::tests::store_helpers::{close_store, close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_scoped_tokens_when_revoked_or_the_run_ends_then_all_replicas_reject_them_and_audit_failure_rolls_back()
 {
    let test_cases = [
        CredentialTestCase {
            description: "SQLite credential lifecycle and atomic audit",
            backend: Backend::Sqlite,
            expected_hash_length: 64,
        },
        CredentialTestCase {
            description: "PostgreSQL credential lifecycle and atomic audit",
            backend: Backend::Postgres,
            expected_hash_length: 64,
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
            assert_eq!(
                authentication_revision(&replica)
                    .await
                    .expect("initial revision"),
                0
            );
            let context = AuditContext {
                actor: "test-admin".into(),
                granting_role: "admin".into(),
                request_id: Uuid::now_v7(),
                ip_address: Some("127.0.0.1".parse().expect("loopback")),
            };
            let mut transaction = begin(store, TransactionMode::Write)
                .await
                .expect("predecessor");
            for name in ["legacy_person_token.sql", "token_rotation.sql"] {
                let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../benchmarks/duckshop/edge_cases/orchestration")
                    .join(name);
                let sql = std::fs::read_to_string(path).expect("DuckShop predecessor tokens");
                for statement in sql.split(';').map(str::trim).filter(|sql| !sql.is_empty()) {
                    (transaction, _) = query(transaction, statement, vec![])
                        .await
                        .expect("synthetic predecessor token");
                }
            }
            commit(transaction).await.expect("predecessor committed");
            let personal_scope = CredentialScope::Principal {
                principal_id: Uuid::parse_str("00000000-0000-4000-8000-000000000001")
                    .expect("synthetic person identity"),
            };
            assert!(
                issue_token(
                    store,
                    &context,
                    personal_scope,
                    Utc::now() + Duration::hours(1)
                )
                .await
                .is_err(),
                "people cannot mint API tokens"
            );
            assert_eq!(
                stored_hashes(&replica).await.len(),
                4,
                "rejected issuance writes nothing"
            );
            for id in [
                "00000000-0000-4000-8000-000000000002",
                "00000000-0000-4000-8000-000000000012",
                "00000000-0000-4000-8000-000000000022",
                "00000000-0000-4000-8000-000000000032",
            ] {
                assert!(
                    rotate_token(
                        store,
                        &context,
                        Uuid::parse_str(id).expect("synthetic token identity")
                    )
                    .await
                    .is_err(),
                    "person, expired, disabled and archived-location credentials cannot be rotated"
                );
            }
            assert_eq!(stored_hashes(&replica).await.len(), 4);
            let legacy_bearer = format!("00000000-0000-4000-8000-000000000002.{}", "0".repeat(64));
            assert!(
                authenticate_token(&replica, &legacy_bearer)
                    .await
                    .expect("predecessor personal credential")
                    .is_none(),
                "pre-existing personal tokens are rejected on every replica"
            );
            let fault = TransactionFault {
                description: "security audit failure",
                table: "sqb_security_audit",
                operation: "INSERT",
                condition: "",
                sqlite_body: "SELECT RAISE(ABORT,'injected audit failure')",
                postgres_body: "RAISE EXCEPTION 'injected audit failure'",
            };
            let revision_fault = TransactionFault {
                description: "authentication revision failure",
                table: "sqb_authentication_revision",
                operation: "UPDATE",
                ..fault
            };
            let insertion_fault = TransactionFault {
                description: "replacement issuance failure",
                table: "sqb_access_tokens",
                ..fault
            };
            let rotation_audit_fault = TransactionFault {
                description: "rotation linkage audit failure",
                condition: "WHEN NEW.action='token.rotated'",
                ..fault
            };
            install_fault(store, &fault).await;
            assert!(
                create_principal(
                    store,
                    &context,
                    ("orders-ci", PrincipalKind::ServiceAccount)
                )
                .await
                .is_err()
            );
            assert!(create_location(store, "orders", &context).await.is_err());
            assert_eq!(location_state(&replica).await, (1, 0));
            remove_fault(store, &fault).await;
            let principal_id = create_principal(
                store,
                &context,
                ("orders-ci", PrincipalKind::ServiceAccount),
            )
            .await
            .expect("principal creation and audit rolled back together");
            let location_id = create_location(store, "orders", &context)
                .await
                .expect("location");
            assert_eq!(location_state(&replica).await, (2, 1));
            let scopes = [
                CredentialScope::Principal { principal_id },
                CredentialScope::Location { location_id },
            ];
            let expiry = Utc::now() + Duration::hours(1);
            for scope in scopes {
                let issued = issue_token(store, &context, scope, expiry)
                    .await
                    .expect("issue scoped token");
                assert_eq!(
                    authenticate_token(&replica, &issued.bearer)
                        .await
                        .expect("replica verifies"),
                    Some(issued.identity.clone())
                );
                assert!(
                    !format!("{issued:?}").contains(&issued.bearer),
                    "debug output redacts secrets"
                );
                let replacement = ["0", "1"][usize::from(issued.bearer.ends_with('0'))];
                let tampered =
                    format!("{}{replacement}", &issued.bearer[..issued.bearer.len() - 1]);
                for invalid in [
                    &tampered,
                    "",
                    "not-a-token",
                    &format!("{}.{}", Uuid::now_v7(), "0".repeat(64)),
                ] {
                    assert!(
                        authenticate_token(&replica, invalid)
                            .await
                            .expect("invalid credential")
                            .is_none()
                    );
                }
                let revision = authentication_revision(&replica)
                    .await
                    .expect("before rotation");
                let count = stored_hashes(store).await.len();
                for failure in [
                    &fault,
                    &insertion_fault,
                    &rotation_audit_fault,
                    &revision_fault,
                ] {
                    install_fault(store, failure).await;
                    assert!(
                        rotate_token(store, &context, issued.identity.token_id)
                            .await
                            .is_err()
                    );
                    remove_fault(store, failure).await;
                    assert_eq!(
                        authenticate_token(&replica, &issued.bearer)
                            .await
                            .expect("old credential survives rollback"),
                        Some(issued.identity.clone())
                    );
                    assert_eq!(stored_hashes(&replica).await.len(), count);
                    assert_eq!(
                        authentication_revision(&replica)
                            .await
                            .expect("rollback revision"),
                        revision
                    );
                }
                let (first, second) = tokio::join!(
                    rotate_token(store, &context, issued.identity.token_id),
                    rotate_token(&replica, &context, issued.identity.token_id),
                );
                assert_ne!(
                    first.is_ok(),
                    second.is_ok(),
                    "concurrent rotations have exactly one winner"
                );
                let rotated = first.or(second).expect("rotation winner");
                assert_eq!(rotated.identity.scope, issued.identity.scope);
                assert_eq!(rotated.identity.expires_at, issued.identity.expires_at);
                assert_ne!(rotated.identity.token_id, issued.identity.token_id);
                assert_ne!(rotated.bearer, issued.bearer);
                assert!(
                    authenticate_token(&replica, &issued.bearer)
                        .await
                        .expect("old token revoked atomically")
                        .is_none()
                );
                assert_eq!(
                    authenticate_token(store, &rotated.bearer)
                        .await
                        .expect("replacement authenticates"),
                    Some(rotated.identity.clone())
                );
                assert_eq!(stored_hashes(&replica).await.len(), count + 1);
                assert_eq!(
                    authentication_revision(&replica)
                        .await
                        .expect("one committed rotation"),
                    revision + 1
                );
                let (transaction, audits) = query(begin(store, TransactionMode::Read).await.expect("read audit"),
                    "SELECT before_json, after_json FROM sqb_security_audit WHERE action='token.rotated'", vec![]).await.expect("rotation audit");
                commit(transaction).await.expect("audit read");
                let audit = audits
                    .iter()
                    .find(|audit| {
                        audit["before_json"]["token_id"]
                            == serde_json::json!(issued.identity.token_id)
                    })
                    .expect("linked rotation history");
                assert_eq!(audit["before_json"], serde_json::json!(issued.identity));
                assert_eq!(audit["after_json"], serde_json::json!(rotated.identity));
                let issued = rotated;
                let revision = authentication_revision(&replica)
                    .await
                    .expect("before revocation");
                for failure in [&fault, &revision_fault] {
                    install_fault(store, failure).await;
                    assert!(
                        revoke_token(store, &context, issued.identity.token_id)
                            .await
                            .is_err()
                    );
                    remove_fault(store, failure).await;
                    assert!(
                        authenticate_token(&replica, &issued.bearer)
                            .await
                            .expect("failed revocation rolled back")
                            .is_some()
                    );
                    assert_eq!(
                        authentication_revision(&replica)
                            .await
                            .expect("rollback revision"),
                        revision
                    );
                }
                assert!(
                    revoke_token(store, &context, issued.identity.token_id)
                        .await
                        .expect("revoke")
                );
                assert_eq!(
                    authentication_revision(&replica)
                        .await
                        .expect("committed revision"),
                    revision + 1
                );
                assert!(
                    !revoke_token(&replica, &context, issued.identity.token_id)
                        .await
                        .expect("retry revoke")
                );
                assert_eq!(
                    authentication_revision(store)
                        .await
                        .expect("idempotent revision"),
                    revision + 1
                );
                assert!(
                    authenticate_token(&replica, &issued.bearer)
                        .await
                        .expect("revoked token")
                        .is_none()
                );
                assert!(
                    issue_token(store, &context, scope, Utc::now() - Duration::seconds(1))
                        .await
                        .is_err()
                );
                let stored = stored_hashes(store).await;
                assert!(
                    stored
                        .iter()
                        .all(|hash| hash.len() == test_case.expected_hash_length
                            && hash != &issued.bearer),
                    "{}",
                    test_case.description
                );
            }
            for scope in [
                CredentialScope::Principal {
                    principal_id: Uuid::now_v7(),
                },
                CredentialScope::Location {
                    location_id: Uuid::now_v7(),
                },
                CredentialScope::Run {
                    run_id: Uuid::now_v7(),
                    claim_id: Uuid::now_v7(),
                },
            ] {
                assert!(
                    issue_token(store, &context, scope, expiry).await.is_err(),
                    "inactive scopes cannot acquire credentials"
                );
            }
            let published = publish_snapshot(store, &snapshot(location_id))
                .await
                .expect("snapshot");
            queue(
                store,
                &request((location_id, published.snapshot_id), 1, 0, &["orders"]),
            )
            .await;
            let lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("elect")
                .expect("leader");
            let claim = claim_next_run(store, &lease, &BTreeMap::new(), None)
                .await
                .expect("claim")
                .expect("work");
            let scope = CredentialScope::Run {
                run_id: claim.request.run_id,
                claim_id: claim.claim_id,
            };
            let issued = issue_token(store, &context, scope, expiry)
                .await
                .expect("mint claimed-run token");
            let rotated = rotate_token(store, &context, issued.identity.token_id)
                .await
                .expect("rotate claimed-run credential");
            assert_eq!(rotated.identity.scope, issued.identity.scope);
            assert_eq!(rotated.identity.expires_at, issued.identity.expires_at);
            assert!(
                authenticate_token(&replica, &issued.bearer)
                    .await
                    .expect("old run credential revoked")
                    .is_none()
            );
            let issued = rotated;
            assert_eq!(
                authenticate_token(&replica, &issued.bearer)
                    .await
                    .expect("worker token"),
                Some(issued.identity.clone())
            );
            assert!(
                issue_token(
                    store,
                    &context,
                    CredentialScope::Run {
                        run_id: claim.request.run_id,
                        claim_id: Uuid::now_v7()
                    },
                    expiry
                )
                .await
                .is_err()
            );
            finish(store, &lease, &claim).await;
            assert!(
                authenticate_token(&replica, &issued.bearer)
                    .await
                    .expect("terminal token expires immediately")
                    .is_none()
            );
            assert!(
                issue_token(store, &context, scope, expiry).await.is_err(),
                "a terminal claim cannot mint a new token"
            );
            assert!(
                rotate_token(store, &context, issued.identity.token_id)
                    .await
                    .is_err(),
                "a terminal claim cannot regain authority through rotation"
            );
            close_store(replica).await;
            close_test_store(fixture).await;
        });
    }
}
