//! Audit expiry deletes only rows every sink received and no retained run's retry still reads.

use serde_json::json;
use uuid::Uuid;

use crate::credentials::main::audit::append_audit::append_audit;
use crate::credentials::main::audit::prune_audit::prune_audit;
use crate::credentials::models::{AuditContext, AuditRetention};
use crate::credentials::tests::helpers::{acknowledge_audit, age_audit, audit_rows};
use crate::credentials::tests::location_helpers::create_location;
use crate::credentials::tests::test_types::AuditRetentionTestCase;
use crate::runs::constants::MANUAL_JOB_AUDIT_TAG;
use crate::runs::main::request_job_run::request_job_run;
use crate::runs::models::JobRunRequest;
use crate::runs::tests::helpers::snapshot;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::store::main::{begin::begin, commit::commit, migrate_store::migrate_store};
use crate::store::models::TransactionMode;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_expired_audit_rows_when_pruned_then_undelivered_and_retry_referenced_rows_survive() {
    let test_cases = [
        AuditRetentionTestCase {
            description: "SQLite keeps undelivered, retry-referenced and newest rows",
            backend: Backend::Sqlite,
            expected_retained: 3,
        },
        AuditRetentionTestCase {
            description: "PostgreSQL keeps undelivered, retry-referenced and newest rows",
            backend: Backend::Postgres,
            expected_retained: 3,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let context = AuditContext {
                actor: "orders-operator".into(),
                granting_role: "control-store-owner".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            let location_id = create_location(store, "orders", "orders-admin")
                .await
                .expect("location");
            let mut input = snapshot(location_id);
            input.documents.insert(
                "jobs".into(),
                json!({"daily_orders": {
                    "description": "Build orders", "selection": ["orders"], "settings": {},
                    "concurrency_groups": []
                }}),
            );
            publish_snapshot(store, &input).await.expect("publish job");
            let delivered = Uuid::now_v7();
            let missing_run = Uuid::now_v7();
            let orphaned = Uuid::new_v5(&missing_run, MANUAL_JOB_AUDIT_TAG);
            for (event_id, scope) in [
                (delivered, json!({"location_id": location_id})),
                (orphaned, json!({"run_id": missing_run})),
            ] {
                let transaction = begin(store, TransactionMode::Write).await.expect("begin");
                let transaction = append_audit(
                    transaction,
                    &context,
                    (event_id, "audit.retention_fixture"),
                    (scope, json!(null), json!({"recorded": true})),
                )
                .await
                .expect("append");
                commit(transaction).await.expect("commit");
            }
            let request = JobRunRequest {
                run_id: Uuid::now_v7(),
                location_id,
                target: "development".into(),
                job_key: "daily_orders".into(),
                reason: "Refresh orders".into(),
                priority: 1,
                window: None,
            };
            assert!(
                request_job_run(store, &context, &request)
                    .await
                    .expect("request")
            );
            let referenced = Uuid::new_v5(&request.run_id, MANUAL_JOB_AUDIT_TAG);
            let (rows, _) = audit_rows(store).await;
            let through = rows.last().expect("referenced row").1;
            let undelivered = Uuid::now_v7();
            let newest = Uuid::now_v7();
            for event_id in [undelivered, newest] {
                let transaction = begin(store, TransactionMode::Write).await.expect("begin");
                let transaction = append_audit(
                    transaction,
                    &context,
                    (event_id, "audit.retention_fixture"),
                    (json!({}), json!(null), json!({"recorded": true})),
                )
                .await
                .expect("append");
                commit(transaction).await.expect("commit");
            }
            age_audit(store, 401).await;
            acknowledge_audit(store, "orders-audit-sink", through).await;
            let sinks = ["orders-audit-sink".to_string()];
            assert!(AuditRetention::days(0).is_err());
            assert_eq!(
                prune_audit(store, AuditRetention::days(500).expect("horizon"), &sinks)
                    .await
                    .expect("longer horizon")
                    .0,
                0
            );
            let two_sinks = [sinks[0].clone(), "inventory-audit-sink".to_string()];
            assert_eq!(
                prune_audit(store, AuditRetention::default(), &two_sinks)
                    .await
                    .expect("second sink has received nothing")
                    .0,
                0
            );
            let total = rows.len() + 2;
            let (deleted, retained) = prune_audit(store, AuditRetention::default(), &sinks)
                .await
                .expect("prune");
            assert_eq!(
                retained, test_case.expected_retained,
                "{}",
                test_case.description
            );
            assert_eq!(deleted, total - 3);
            let (remaining, deliveries) = audit_rows(store).await;
            let ids: Vec<Uuid> = remaining.iter().map(|(id, _)| *id).collect();
            assert_eq!(ids, [referenced, undelivered, newest]);
            assert_eq!(deliveries, 1, "only the retained row's delivery remains");
            assert!(
                !request_job_run(store, &context, &request)
                    .await
                    .expect("retry still finds its original request")
            );
            let transaction = begin(store, TransactionMode::Write).await.expect("begin");
            let transaction = append_audit(
                transaction,
                &context,
                (Uuid::now_v7(), "audit.retention_fixture"),
                (json!({}), json!(null), json!({"recorded": true})),
            )
            .await
            .expect("append after prune");
            commit(transaction).await.expect("commit");
            let (after, _) = audit_rows(store).await;
            assert_eq!(
                after.last().expect("new row").1,
                remaining.last().expect("newest").1 + 1,
                "storage IDs are never reused"
            );
            assert_eq!(
                prune_audit(store, AuditRetention::default(), &sinks)
                    .await
                    .expect("idempotent"),
                (0, test_case.expected_retained)
            );
            close_test_store(fixture).await;
        });
    }
}
