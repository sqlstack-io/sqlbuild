//! A wide group forces multiple predicate chunks; the last node is not optional.

use std::time::Duration;

use serde_json::json;
use uuid::Uuid;

use crate::credentials::models::AuditContext;
use crate::credentials::tests::location_helpers::create_location;
use crate::declarations::main::list_triggers::list_triggers;
use crate::runs::main::{
    advance_groups::advance_groups, change_run::change_run, claim_next_run::claim_next_run,
    request_job_run::request_job_run,
};
use crate::runs::models::{
    JobRunRequest, NodeOutcome, RunAuthority, RunChange, RunChangeRequest, RunOutcome,
};
use crate::scheduler::main::renew_lease::renew_lease;
use crate::scheduler::tests::contracts::helpers::acquire_lease;
use crate::sensors::main::{
    begin_sensor_evaluation::begin_sensor_evaluation,
    complete_builtin_sensor::complete_builtin_sensor,
};
use crate::sensors::tests::helpers::snapshot;
use crate::sensors::tests::test_types::SensorTestCase;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_a_wide_group_when_only_the_last_node_has_not_started_then_all_chunks_must_match_within_a_bounded_evaluation()
 {
    let test_cases = [
        SensorTestCase {
            description: "SQLite wide executed-node predicate",
            backend: Backend::Sqlite,
            expected_runs: 1,
        },
        SensorTestCase {
            description: "PostgreSQL wide executed-node predicate",
            backend: Backend::Postgres,
            expected_runs: 1,
        },
    ];
    for test_case in test_cases {
        run(async {
            let fixture = open_test_store(test_case.backend)
                .await
                .expect("required backend");
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let location = create_location(store, "orders", "test-admin")
                .await
                .expect("location");
            let mut input = snapshot(location);
            let node = input.nodes["orders"].clone();
            let selection: Vec<String> = (0..501)
                .map(|index| format!("model:orders_{index:04}"))
                .collect();
            input
                .nodes
                .extend(selection.iter().map(|key| (key.clone(), node.clone())));
            input.documents.insert("jobs".into(), json!({
                "source": {"description": "Wide source", "selection": selection, "overlap": "allow"},
                "destination": {"description": "Destination", "selection": ["orders"], "overlap": "allow"}
            }));
            input.documents.insert("groups".into(), json!({
                "wide_orders": {"description": "Wide group", "selection": selection, "jobs": {"source": []}, "overlap": "allow"}
            }));
            input.documents.insert("triggers".into(), json!({
                "executed": {"description": "All source nodes must execute", "job_key": "destination",
                    "when": {"kind": "run_event", "job_key": "wide_orders", "kinds": ["completed"], "executed_successfully": selection},
                    "minimum_interval_seconds": 1}
            }));
            publish_snapshot(store, &input)
                .await
                .expect("publish wide predicate");
            let lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("elect")
                .expect("leader");
            let limits = std::collections::BTreeMap::new();
            let scope = (location, "development");
            for include_last in [true, false] {
                let parent = Uuid::now_v7();
                let context = AuditContext {
                    actor: "test-admin".into(),
                    granting_role: "control-store-owner".into(),
                    request_id: Uuid::now_v7(),
                    ip_address: None,
                };
                request_job_run(
                    store,
                    &context,
                    &JobRunRequest {
                        run_id: parent,
                        location_id: location,
                        target: "development".into(),
                        job_key: "wide_orders".into(),
                        reason: "Exercise wide execution evidence".into(),
                        priority: 10,
                        window: None,
                    },
                )
                .await
                .expect("request");
                advance_groups(store, &lease, (&limits, scope))
                    .await
                    .expect("admit parent");
                let claim = claim_next_run(store, &lease, &limits, Some(scope))
                    .await
                    .expect("claim")
                    .expect("child");
                assert_eq!(claim.request.parent_run_id, Some(parent));
                let changes = std::iter::once(RunChange::Started)
                    .chain(
                        selection
                            .iter()
                            .filter(|key| include_last || key.as_str() != "model:orders_0500")
                            .map(|key| RunChange::NodeStarted {
                                node_key: key.clone(),
                            }),
                    )
                    .chain(selection.iter().map(|key| RunChange::NodeFinished {
                        node_key: key.clone(),
                        outcome: NodeOutcome::Success,
                        rows_affected: None,
                        error_code: None,
                        skipped_because_of: None,
                    }))
                    .chain(std::iter::once(RunChange::Finished {
                        outcome: RunOutcome::Success,
                        reason: "All logical nodes succeed".into(),
                        exit_code: None,
                    }))
                    .collect::<Vec<_>>();
                for batch in changes.chunks(100) {
                    // Evidence setup must keep its scheduler alive, just like the real loop.
                    renew_lease(store, &lease)
                        .await
                        .expect("renew during setup");
                    for change in batch {
                        change_run(
                            store,
                            RunAuthority::Worker {
                                run_id: claim.request.run_id,
                                claim_id: claim.claim_id,
                            },
                            &RunChangeRequest {
                                event_id: Uuid::now_v7(),
                                run_id: claim.request.run_id,
                                claim_id: Some(claim.claim_id),
                                change: change.clone(),
                            },
                        )
                        .await
                        .expect("real node evidence");
                    }
                }
                advance_groups(store, &lease, (&limits, scope))
                    .await
                    .expect("finish parent");
            }
            let trigger = list_triggers(store, location, "development")
                .await
                .expect("triggers")
                .remove(0);
            let evaluation = begin_sensor_evaluation(store, &lease, trigger.trigger_id)
                .await
                .expect("request evaluation")
                .expect("due");
            let count = tokio::time::timeout(
                Duration::from_secs(5),
                complete_builtin_sensor(store, &lease, &evaluation),
            )
            .await
            .expect("wide predicate must not hold the write lock unboundedly")
            .expect("complete");
            assert_eq!(count, test_case.expected_runs, "{}", test_case.description);
            close_test_store(fixture).await;
        });
    }
}
