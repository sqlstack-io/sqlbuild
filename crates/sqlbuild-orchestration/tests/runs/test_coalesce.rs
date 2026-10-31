use std::collections::BTreeMap;

use serde_json::json;
use sqlbuild_orchestration::declarations::main::list_triggers::list_triggers;
use sqlbuild_orchestration::runs::main::claim_next_run::claim_next_run;
use sqlbuild_orchestration::runs::main::list_runs::list_runs;
use sqlbuild_orchestration::scheduler::main::acquire_lease::acquire_lease;
use sqlbuild_orchestration::scheduler::main::evaluate_trigger::evaluate_trigger;
use sqlbuild_orchestration::scheduler::models::TickDecision;
use sqlbuild_orchestration::snapshots::main::publish_snapshot::publish_snapshot;
use sqlbuild_orchestration::store::main::migrate_store::migrate_store;
use uuid::Uuid;

use crate::helpers::snapshot;
use crate::location_helpers::create_location;
use crate::store_helpers::{close_test_store, open_test_store, run};
use crate::store_test_types::Backend;
use crate::test_types::CoalescedRunTestCase;

#[test]
fn given_coalescing_triggers_when_a_previous_run_has_been_claimed_then_new_ticks_wait_in_a_new_run_instead_of_covering_executing_work()
 {
    let test_cases = [
        CoalescedRunTestCase {
            description: "SQLite coalesces only work that has not been claimed",
            backend: Backend::Sqlite,
            expected_runs: 2,
        },
        CoalescedRunTestCase {
            description: "PostgreSQL coalesces only work that has not been claimed",
            backend: Backend::Postgres,
            expected_runs: 2,
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
            let mut input = snapshot(location);
            input.documents.insert(
                "jobs".into(),
                json!({"orders": {"description": "Build orders", "selection": ["orders"]}}),
            );
            input.documents.insert("triggers".into(), json!({"daily_orders": {"description": "Refresh orders", "job_key": "orders", "starts_at": "2020-01-01T00:00:00Z",
                "schedule": {"kind": "calendar", "times": ["2020-01-01T00:00:00Z"], "refresh_at": "2030-01-01T00:00:00Z"}}}));
            let published = publish_snapshot(store, &input)
                .await
                .expect("first calendar publication");
            let trigger = list_triggers(store, location, "development")
                .await
                .expect("trigger")[0]
                .trigger_id;
            let lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("elect")
                .expect("leader");
            evaluate_trigger(store, &lease, trigger)
                .await
                .expect("first tick");
            let claim = claim_next_run(store, &lease, &BTreeMap::new(), None)
                .await
                .expect("claim first run")
                .expect("scheduled run");
            assert_eq!(claim.request.snapshot_id, published.snapshot_id);
            input.generation = 2;
            input.documents.get_mut("triggers").expect("triggers")["daily_orders"]["schedule"]["times"] =
                json!(["2020-01-01T00:00:00Z", "2020-01-02T00:00:00Z"]);
            publish_snapshot(store, &input)
                .await
                .expect("refresh calendar");
            let plan = evaluate_trigger(store, &lease, trigger)
                .await
                .expect("second tick")
                .expect("active trigger");
            assert_eq!(plan.actions.len(), 1);
            let TickDecision::Requested { run_id: waiting } = plan.actions[0].decision else {
                panic!("future work must not be covered by an already claimed run");
            };
            assert_ne!(waiting, claim.request.run_id);
            input.generation = 3;
            input.documents.get_mut("triggers").expect("triggers")["daily_orders"]["schedule"]["times"] =
                json!([
                    "2020-01-01T00:00:00Z",
                    "2020-01-02T00:00:00Z",
                    "2020-01-03T00:00:00Z"
                ]);
            publish_snapshot(store, &input)
                .await
                .expect("next calendar refresh");
            let plan = evaluate_trigger(store, &lease, trigger)
                .await
                .expect("third tick")
                .expect("active trigger");
            assert_eq!(
                plan.actions[0].decision,
                TickDecision::Covered {
                    covered_by_run_id: waiting
                }
            );
            assert_eq!(
                list_runs(store, (location, "development"), 100)
                    .await
                    .expect("one executing and one waiting")
                    .len(),
                test_case.expected_runs,
                "{}",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}
