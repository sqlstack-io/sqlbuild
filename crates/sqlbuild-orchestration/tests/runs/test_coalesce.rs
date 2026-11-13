use std::collections::BTreeMap;

use crate::declarations::main::list_triggers::list_triggers;
use crate::runs::main::claim_next_run::claim_next_run;
use crate::runs::main::list_runs::list_runs;
use crate::runs::tests::claim_helpers::succeed;
use crate::scheduler::main::list_ticks::list_ticks;
use crate::scheduler::models::{TickDecision, TickOutcome};
use crate::scheduler::tests::contracts::helpers::{acquire_lease, evaluate_trigger};
use crate::scheduler::tests::contracts::ticks::helpers::calendar_text;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::store::main::migrate_store::migrate_store;
use serde_json::json;
use uuid::Uuid;

use crate::credentials::tests::location_helpers::create_location;
use crate::runs::tests::helpers::snapshot;
use crate::runs::tests::test_types::CoalescedRunTestCase;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

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
            input.documents.insert("triggers".into(), json!({"daily_orders": {"description": "Refresh orders", "job_key": "orders", "starts_at": calendar_text(0),
                "schedule": {"kind": "calendar", "times": [calendar_text(0)], "refresh_at": "2030-01-01T00:00:00Z"}}}));
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
                json!([calendar_text(0), calendar_text(1)]);
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
                json!([calendar_text(0), calendar_text(1), calendar_text(2)]);
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

#[test]
fn given_a_skip_overlap_job_when_a_tick_falls_due_during_its_run_then_the_tick_is_skipped_and_the_next_one_runs()
 {
    let test_cases = [
        CoalescedRunTestCase {
            description: "SQLite skips an on-time tick while the job runs",
            backend: Backend::Sqlite,
            expected_runs: 2,
        },
        CoalescedRunTestCase {
            description: "PostgreSQL skips an on-time tick while the job runs",
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
                json!({"orders": {"description": "Build orders", "selection": ["orders"], "overlap": "skip"}}),
            );
            input.documents.insert("triggers".into(), json!({"daily_orders": {"description": "Refresh orders", "job_key": "orders", "starts_at": calendar_text(0),
                "schedule": {"kind": "calendar", "times": [calendar_text(0)], "refresh_at": "2030-01-01T00:00:00Z"}}}));
            publish_snapshot(store, &input).await.expect("publish");
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
            input.generation = 2;
            input.documents.get_mut("triggers").expect("triggers")["daily_orders"]["schedule"]["times"] =
                json!((0..2).map(calendar_text).collect::<Vec<_>>());
            publish_snapshot(store, &input)
                .await
                .expect("add a second calendar time");
            let plan = evaluate_trigger(store, &lease, trigger)
                .await
                .expect("tick while the first run executes")
                .expect("active trigger");
            assert_eq!(
                plan.actions[0].decision,
                TickDecision::Skipped {
                    active_run_id: claim.request.run_id
                },
                "{}",
                test_case.description
            );
            succeed(store, &claim).await;
            input.generation = 3;
            input.documents.get_mut("triggers").expect("triggers")["daily_orders"]["schedule"]["times"] =
                json!((0..3).map(calendar_text).collect::<Vec<_>>());
            publish_snapshot(store, &input)
                .await
                .expect("add a third calendar time");
            let plan = evaluate_trigger(store, &lease, trigger)
                .await
                .expect("tick after the first run ended")
                .expect("active trigger");
            assert!(
                matches!(plan.actions[0].decision, TickDecision::Requested { .. }),
                "{}: the job ended, so the next tick queues",
                test_case.description
            );
            let ticks = list_ticks(store, trigger, 10).await.expect("ticks");
            let skipped: Vec<_> = ticks
                .iter()
                .filter(|tick| tick.outcome == TickOutcome::Skipped)
                .map(|tick| (tick.active_run_id, tick.run_id))
                .collect();
            assert_eq!(
                skipped,
                vec![(Some(claim.request.run_id), None)],
                "{}",
                test_case.description
            );
            assert_eq!(
                list_runs(store, (location, "development"), 100)
                    .await
                    .expect("runs")
                    .len(),
                test_case.expected_runs,
                "{}: a skipped tick creates no run",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}
