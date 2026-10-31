use std::collections::BTreeMap;

use chrono::{Duration, Utc};
use sqlbuild_orchestration::credentials::main::authenticate_token::authenticate_token;
use sqlbuild_orchestration::credentials::main::create_location::create_location;
use sqlbuild_orchestration::credentials::main::create_principal::create_principal;
use sqlbuild_orchestration::credentials::main::issue_token::issue_token;
use sqlbuild_orchestration::credentials::main::revoke_token::revoke_token;
use sqlbuild_orchestration::credentials::models::{AuditContext, CredentialScope, PrincipalKind};
use sqlbuild_orchestration::runs::main::claim_next_run::claim_next_run;
use sqlbuild_orchestration::scheduler::main::acquire_lease::acquire_lease;
use sqlbuild_orchestration::snapshots::main::publish_snapshot::publish_snapshot;
use sqlbuild_orchestration::store::main::migrate_store::migrate_store;
use uuid::Uuid;

use crate::fault_helpers::{install_fault, remove_fault};
use crate::helpers::{location_state, stored_hashes};
use crate::replica_helpers::second_store;
use crate::run_helpers::{queue, request, snapshot};
use crate::store_helpers::{close_store, close_test_store, open_test_store, run};
use crate::store_test_types::Backend;
use crate::terminal_helpers::finish;
use crate::test_types::{CredentialTestCase, TransactionFault};

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
            let context = AuditContext {
                actor: "test-admin".into(),
                granting_role: "admin".into(),
                request_id: Uuid::now_v7(),
                ip_address: Some("127.0.0.1".parse().expect("loopback")),
            };
            let fault = TransactionFault {
                description: "security audit failure",
                table: "sqb_security_audit",
                operation: "INSERT",
                condition: "",
                sqlite_body: "SELECT RAISE(ABORT,'injected audit failure')",
                postgres_body: "RAISE EXCEPTION 'injected audit failure'",
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
            assert_eq!(location_state(&replica).await, (0, 0));
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
            assert_eq!(location_state(&replica).await, (1, 1));
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
                install_fault(store, &fault).await;
                assert!(
                    revoke_token(store, &context, issued.identity.token_id)
                        .await
                        .is_err()
                );
                remove_fault(store, &fault).await;
                assert!(
                    authenticate_token(&replica, &issued.bearer)
                        .await
                        .expect("failed revocation rolled back")
                        .is_some()
                );
                assert!(
                    revoke_token(store, &context, issued.identity.token_id)
                        .await
                        .expect("revoke")
                );
                assert!(
                    !revoke_token(&replica, &context, issued.identity.token_id)
                        .await
                        .expect("retry revoke")
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
            close_store(replica).await;
            close_test_store(fixture).await;
        });
    }
}
