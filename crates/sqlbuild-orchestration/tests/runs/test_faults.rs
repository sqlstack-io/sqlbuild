use std::collections::BTreeMap;
use std::num::NonZeroU32;

use sqlbuild_orchestration::runs::main::change_run::change_run;
use sqlbuild_orchestration::runs::main::claim_next_run::claim_next_run;
use sqlbuild_orchestration::runs::main::heartbeat_run::heartbeat_run;
use sqlbuild_orchestration::runs::main::list_runs::list_runs;
use sqlbuild_orchestration::runs::models::{RunAuthority, RunChange, RunChangeRequest};
use sqlbuild_orchestration::scheduler::main::acquire_lease::acquire_lease;
use sqlbuild_orchestration::snapshots::main::publish_snapshot::publish_snapshot;
use sqlbuild_orchestration::store::errors::ControlError;
use sqlbuild_orchestration::store::main::migrate_store::migrate_store;
use sqlbuild_orchestration::store::main::read_recall_events::read_recall_events;
use uuid::Uuid;

use crate::fault_helpers::{install_fault, remove_fault};
use crate::helpers::{queue, request, snapshot};
use crate::lease_helpers::expire_lease;
use crate::location_helpers::create_location;
use crate::store_helpers::{close_test_store, open_test_store, run};
use crate::store_test_types::Backend;
use crate::test_types::{ClaimFailureTestCase, TransactionFault};

#[test]
fn given_failures_during_claim_publication_when_retried_then_claims_slots_liveness_facts_and_projection_commit_together()
 {
    let test_cases = [
        ClaimFailureTestCase {
            description: "SQLite rolls back every claim-publication failure window",
            backend: Backend::Sqlite,
            expected_run: 1,
        },
        ClaimFailureTestCase {
            description: "PostgreSQL rolls back every claim-publication failure window",
            backend: Backend::Postgres,
            expected_run: 1,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let location = create_location(store, "orders", "test-admin")
                .await
                .expect("location");
            let published = publish_snapshot(store, &snapshot(location))
                .await
                .expect("snapshot");
            let mut input = request(
                (location, published.snapshot_id),
                1,
                1,
                &["customers", "orders"],
            );
            input.concurrency_groups.insert("warehouse".into());
            queue(store, &input).await;
            let lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("elect")
                .expect("leader");
            let limits = BTreeMap::from([(
                "warehouse".into(),
                NonZeroU32::new(1).expect("positive limit"),
            )]);
            let facts = read_recall_events(store, 0, 100)
                .await
                .expect("requested and queued facts");
            let projected = list_runs(store, (location, "development"), 100)
                .await
                .expect("queued projection");
            for (table, operation) in [
                ("sqb_write_claims", "INSERT"),
                ("sqb_concurrency_slots", "INSERT"),
                ("sqb_run_liveness", "INSERT"),
                ("recall_events", "INSERT"),
                ("sqb_runs", "UPDATE"),
            ] {
                let point = TransactionFault {
                    description: "claim publication failure",
                    table,
                    operation,
                    condition: "",
                    sqlite_body: "SELECT RAISE(ABORT,'injected claim failure')",
                    postgres_body: "RAISE EXCEPTION 'injected claim failure'",
                };
                install_fault(store, &point).await;
                assert!(
                    claim_next_run(store, &lease, &limits, None).await.is_err(),
                    "{} at {table}",
                    point.description
                );
                remove_fault(store, &point).await;
                assert_eq!(
                    read_recall_events(store, 0, 100)
                        .await
                        .expect("rolled back claim fact"),
                    facts
                );
                assert_eq!(
                    list_runs(store, (location, "development"), 100)
                        .await
                        .expect("rolled back claim projection"),
                    projected
                );
            }
            let expiry = TransactionFault {
                description: "claim fence expires before commit",
                table: "sqb_run_liveness",
                operation: "INSERT",
                condition: "",
                sqlite_body: "UPDATE sqb_scheduler_lease SET expires_at='2000-01-01T00:00:00.000000Z'",
                postgres_body: "UPDATE sqb_scheduler_lease SET expires_at='2000-01-01T00:00:00.000000Z'",
            };
            install_fault(store, &expiry).await;
            assert!(
                matches!(
                    claim_next_run(store, &lease, &limits, None).await,
                    Err(ControlError::Fenced)
                ),
                "{}",
                expiry.description
            );
            remove_fault(store, &expiry).await;
            let claim = claim_next_run(store, &lease, &limits, None)
                .await
                .expect("retry complete claim")
                .expect("no leaked claims or slots");
            assert_eq!(claim.request, input);
            assert_eq!(
                claim.request.run_id,
                Uuid::from_u128(test_case.expected_run),
                "{}",
                test_case.description
            );
            assert!(
                claim_next_run(store, &lease, &limits, None)
                    .await
                    .expect("already claimed run is not relaunched")
                    .is_none()
            );
            expire_lease(store).await;
            let next = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("take over")
                .expect("new leader");
            let launched = RunChangeRequest {
                event_id: Uuid::now_v7(),
                run_id: input.run_id,
                claim_id: Some(claim.claim_id),
                change: RunChange::Launched {
                    container_id: Some("synthetic-orders-worker".into()),
                },
            };
            assert!(
                matches!(
                    change_run(store, RunAuthority::Scheduler(&lease), &launched).await,
                    Err(ControlError::Fenced)
                ),
                "a former leader cannot acknowledge a launch after takeover"
            );
            assert!(
                change_run(store, RunAuthority::Scheduler(&next), &launched)
                    .await
                    .expect("successor reconciles the existing claim")
            );
            assert!(
                !heartbeat_run(store, (input.run_id, claim.claim_id))
                    .await
                    .expect("workers remain alive through scheduler failover")
            );
            close_test_store(fixture).await;
        });
    }
}
