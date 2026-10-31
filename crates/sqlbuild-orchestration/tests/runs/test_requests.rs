//! Published-job requests freeze execution inputs and audit atomically on both databases.

use std::collections::BTreeMap;

use serde_json::json;
use sqlbuild_orchestration::credentials::models::AuditContext;
use sqlbuild_orchestration::runs::main::claim_next_run::claim_next_run;
use sqlbuild_orchestration::runs::main::list_runs::list_runs;
use sqlbuild_orchestration::runs::main::request_job_run::request_job_run;
use sqlbuild_orchestration::runs::models::{JobRunRequest, RunStatus};
use sqlbuild_orchestration::scheduler::main::acquire_lease::acquire_lease;
use sqlbuild_orchestration::snapshots::main::publish_snapshot::publish_snapshot;
use sqlbuild_orchestration::store::main::migrate_store::migrate_store;
use sqlbuild_orchestration::store::main::read_recall_events::read_recall_events;
use uuid::Uuid;

use crate::fault_helpers::{install_fault, remove_fault};
use crate::helpers::snapshot;
use crate::location_helpers::create_location;
use crate::store_helpers::{close_test_store, open_test_store, run};
use crate::store_test_types::Backend;
use crate::test_types::{RunLifecycleTestCase, TransactionFault};

#[test]
fn given_a_published_job_when_requested_and_retried_after_redeploy_then_audit_and_frozen_execution_commit_once()
 {
    let test_cases = [
        RunLifecycleTestCase {
            description: "SQLite queues and audits the pinned job exactly once",
            backend: Backend::Sqlite,
            expected_status: RunStatus::Queued,
        },
        RunLifecycleTestCase {
            description: "PostgreSQL queues and audits the pinned job exactly once",
            backend: Backend::Postgres,
            expected_status: RunStatus::Queued,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let location_id = create_location(store, "orders", "test-admin")
                .await
                .expect("location");
            let context = AuditContext {
                actor: "test-operator".into(),
                granting_role: "control-store-owner".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            let mut input = snapshot(location_id);
            let mut source = input.nodes["orders"].clone();
            source.kind = "source".into();
            input.nodes.insert("warehouse_orders".into(), source);
            input.documents.insert("jobs".into(), json!({"daily_orders": {
                "description": "Build orders from the warehouse source", "selection": ["orders", "warehouse_orders"],
                "settings": {"full_refresh": true}, "concurrency_groups": ["orders-load"], "overlap": "queue"
            }}));
            let published = publish_snapshot(store, &input).await.expect("publish job");
            let request = JobRunRequest {
                run_id: Uuid::now_v7(),
                location_id,
                target: "development".into(),
                job_key: "daily_orders".into(),
                reason: "Refresh orders".into(),
                priority: 7,
            };
            for invalid in [
                JobRunRequest {
                    target: "production".into(),
                    ..request.clone()
                },
                JobRunRequest {
                    location_id: Uuid::now_v7(),
                    ..request.clone()
                },
                JobRunRequest {
                    job_key: "unknown_job".into(),
                    ..request.clone()
                },
                JobRunRequest {
                    reason: " ".into(),
                    ..request.clone()
                },
            ] {
                assert!(request_job_run(store, &context, &invalid).await.is_err());
            }
            let fault = TransactionFault {
                description: "manual run audit failure",
                table: "sqb_security_audit",
                operation: "INSERT",
                condition: "",
                sqlite_body: "SELECT RAISE(ABORT,'injected audit failure')",
                postgres_body: "RAISE EXCEPTION 'injected audit failure'",
            };
            install_fault(store, &fault).await;
            assert!(request_job_run(store, &context, &request).await.is_err());
            remove_fault(store, &fault).await;
            assert!(
                list_runs(store, (location_id, "development"), 10)
                    .await
                    .expect("rolled back projection")
                    .is_empty()
            );
            assert!(
                read_recall_events(store, 0, 100)
                    .await
                    .expect("rolled back facts")
                    .is_empty()
            );
            let (first, retry) = tokio::join!(
                request_job_run(store, &context, &request),
                request_job_run(store, &context, &request)
            );
            assert_ne!(
                first.expect("first request"),
                retry.expect("concurrent retry")
            );
            let facts = read_recall_events(store, 0, 100)
                .await
                .expect("request facts");
            assert_eq!(facts.len(), 2);
            input.generation = 2;
            input.image_id = Some("sha256:synthetic-orders-image-2".into());
            input.documents.get_mut("jobs").expect("jobs")["daily_orders"]["selection"] =
                json!(["inventory"]);
            input.documents.get_mut("jobs").expect("jobs")["daily_orders"]["settings"] =
                json!({"full_refresh": false});
            publish_snapshot(store, &input).await.expect("redeploy");
            assert!(
                !request_job_run(store, &context, &request)
                    .await
                    .expect("retry keeps original snapshot")
            );
            let mut changed = request.clone();
            changed.reason = "Different request".into();
            assert!(request_job_run(store, &context, &changed).await.is_err());
            let other_actor = AuditContext {
                actor: "other-operator".into(),
                ..context.clone()
            };
            assert!(
                request_job_run(store, &other_actor, &request)
                    .await
                    .is_err()
            );
            assert_eq!(
                read_recall_events(store, 0, 100)
                    .await
                    .expect("no duplicate facts"),
                facts
            );
            let lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("elect")
                .expect("leader");
            let limits = BTreeMap::from([(
                "orders-load".into(),
                std::num::NonZeroU32::new(1).expect("positive group limit"),
            )]);
            let claim = claim_next_run(store, &lease, &limits, None)
                .await
                .expect("claim")
                .expect("queued request");
            assert_eq!(claim.request.run_id, request.run_id);
            assert_eq!(claim.request.snapshot_id, published.snapshot_id);
            assert_eq!(
                claim.request.image_id.as_deref(),
                Some("sha256:synthetic-orders-image")
            );
            assert_eq!(
                claim.request.selection,
                ["orders".into(), "warehouse_orders".into()].into()
            );
            assert_eq!(claim.request.write_set, ["orders".into()].into());
            assert_eq!(
                claim.request.settings,
                BTreeMap::from([("full_refresh".into(), json!(true))])
            );
            assert_eq!(
                claim.request.concurrency_groups,
                ["orders-load".into()].into()
            );
            assert_eq!(claim.request.priority, 7);
            assert_eq!(claim.request.requested_by, "test-operator");
            let fresh = JobRunRequest {
                run_id: Uuid::now_v7(),
                ..request.clone()
            };
            assert!(
                request_job_run(store, &context, &fresh)
                    .await
                    .expect("new request uses redeploy")
            );
            let runs = list_runs(store, (location_id, "development"), 10)
                .await
                .expect("queued jobs");
            assert_eq!(runs.len(), 2);
            assert_eq!(
                runs.iter()
                    .find(|run| run.run_id == fresh.run_id)
                    .expect("new run")
                    .status,
                test_case.expected_status,
                "{}",
                test_case.description
            );
            assert_eq!(
                runs.iter()
                    .find(|run| run.run_id == fresh.run_id)
                    .expect("new run")
                    .selection_json,
                ["inventory".into()].into()
            );
            close_test_store(fixture).await;
        });
    }
}
