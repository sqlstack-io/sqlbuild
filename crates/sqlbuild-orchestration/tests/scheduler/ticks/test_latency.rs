//! Regression bounds for tick-to-decision latency and a waiting standby's catch-up after a lease lapses.
//!
//! A decision is the durable tick: it queues a run or records the waiting run that covers it.

use chrono::{DurationRound, SecondsFormat, TimeDelta, Utc};
use serde_json::json;

use crate::credentials::tests::location_helpers::create_location;
use crate::declarations::main::list_triggers::list_triggers;
use crate::scheduler::tests::contracts::ticks::helpers::{
    queued_times, run_scheduler_until, snapshot,
};
use crate::scheduler::tests::contracts::ticks::test_types::SchedulerLatencyTestCase;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::lease_helpers::expire_lease;
use crate::store::tests::replica_helpers::second_store;
use crate::store::tests::store_helpers::{close_store, close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_a_running_scheduler_and_a_stopped_leader_when_times_fall_due_then_ticks_queue_within_their_latency_bounds()
 {
    let test_cases = [
        SchedulerLatencyTestCase {
            description: "SQLite scheduler latency",
            backend: Backend::Sqlite,
            expected_ticks: 3,
            expected_ticks_after_failover: 5,
            expected_max_tick_to_decision: TimeDelta::milliseconds(500),
            expected_max_failover_catch_up: TimeDelta::milliseconds(1_500),
        },
        SchedulerLatencyTestCase {
            description: "PostgreSQL scheduler latency",
            backend: Backend::Postgres,
            expected_ticks: 3,
            expected_ticks_after_failover: 5,
            expected_max_tick_to_decision: TimeDelta::milliseconds(500),
            expected_max_failover_catch_up: TimeDelta::milliseconds(1_500),
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
            let base = Utc::now()
                .duration_trunc(TimeDelta::seconds(1))
                .expect("second");
            let at = |seconds| base + TimeDelta::seconds(seconds);
            let text = |seconds| at(seconds).to_rfc3339_opts(SecondsFormat::Secs, true);
            let mut input = snapshot(location);
            input.documents.insert(
                "jobs".into(),
                json!({"orders": {"description": "Build orders", "selection": ["orders"]}}),
            );
            input.documents.insert(
                "triggers".into(),
                json!({"calendar_orders": {"description": "Calendar orders", "job_key": "orders",
                    "schedule": {"kind": "calendar", "times": [text(2), text(3), text(4), text(8), text(9)],
                    "refresh_at": "2029-01-01T00:00:00Z"}}}),
            );
            publish_snapshot(store, &input).await.expect("publication");
            let trigger_id = list_triggers(store, location, "development")
                .await
                .expect("trigger")[0]
                .trigger_id;
            let ticks = run_scheduler_until(store, trigger_id, at(6)).await;
            let queued = queued_times(store, location).await;
            let earliest = ticks
                .iter()
                .min_by_key(|tick| tick.scheduled_for)
                .expect("ticks");
            let requested = earliest.run_id.expect("the first due time queues a run");
            assert!(
                queued[&requested] <= earliest.evaluated_at,
                "{}: the run is queued with its tick",
                test_case.description
            );
            assert!(
                ticks
                    .iter()
                    .filter(|tick| tick.tick_id != earliest.tick_id)
                    .all(|tick| tick.covered_by_run_id == Some(requested)),
                "{}: later times are covered by the waiting run",
                test_case.description
            );
            let latencies: Vec<TimeDelta> = ticks
                .iter()
                .map(|tick| tick.evaluated_at - tick.scheduled_for)
                .collect();
            println!("{} tick-to-decision: {latencies:?}", test_case.description);
            assert_eq!(
                latencies.len(),
                test_case.expected_ticks,
                "{}",
                test_case.description
            );
            assert!(
                latencies
                    .iter()
                    .all(|latency| *latency <= test_case.expected_max_tick_to_decision),
                "{}: {latencies:?}",
                test_case.description
            );
            // The first leader stopped without releasing its lease while two more times fell due.
            // A standby is already polling when that lease lapses, so its own retry wait counts.
            tokio::time::sleep((at(10) - Utc::now()).to_std().unwrap_or_default()).await;
            let second = second_store(store).await;
            let lapse = async {
                tokio::time::sleep((at(11) - Utc::now()).to_std().unwrap_or_default()).await;
                expire_lease(store).await;
                Utc::now()
            };
            let (ticks, available) =
                tokio::join!(run_scheduler_until(&second, trigger_id, at(14)), lapse);
            assert_eq!(
                ticks.len(),
                test_case.expected_ticks_after_failover,
                "{}: the standby ticks both overdue times",
                test_case.description
            );
            let latest = ticks
                .iter()
                .find(|tick| tick.scheduled_for == at(9))
                .expect("the latest overdue time is decided");
            let catch_up = latest.evaluated_at - available;
            println!("{} failover catch-up: {catch_up:?}", test_case.description);
            assert!(
                catch_up <= test_case.expected_max_failover_catch_up,
                "{}: {catch_up:?}",
                test_case.description
            );
            close_store(second).await;
            close_test_store(fixture).await;
        });
    }
}
