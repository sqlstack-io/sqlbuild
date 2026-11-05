//! A sub-second trigger start or cursor never moves a cron tick off its whole-second occurrence.

use std::collections::BTreeSet;

use chrono::{DateTime, DurationRound, TimeDelta, Timelike, Utc};
use serde_json::json;
use uuid::Uuid;

use crate::credentials::tests::location_helpers::create_location;
use crate::declarations::main::list_triggers::list_triggers;
use crate::scheduler::main::{_acquire_lease::acquire_lease, _evaluate_trigger::evaluate_trigger};
use crate::scheduler::tests::contracts::ticks::helpers::{set_trigger_state, snapshot, tick_times};
use crate::scheduler::tests::contracts::ticks::test_types::FirstTickPrecisionTestCase;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_a_sub_second_trigger_start_or_cursor_when_evaluated_then_cron_ticks_land_on_whole_seconds_once()
 {
    let test_cases = [
        FirstTickPrecisionTestCase {
            description: "SQLite first and resumed cron ticks",
            backend: Backend::Sqlite,
            expected_first_tick_minutes_after_start: 1,
            expected_fractional_ticks: 0,
            expected_repeated_occurrences: 0,
        },
        FirstTickPrecisionTestCase {
            description: "PostgreSQL first and resumed cron ticks",
            backend: Backend::Postgres,
            expected_first_tick_minutes_after_start: 1,
            expected_fractional_ticks: 0,
            expected_repeated_occurrences: 0,
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
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
                "../../benchmarks/duckshop/edge_cases/orchestration/first_tick_precision.json",
            );
            let case: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(path).expect("precision fixture"))
                    .expect("precision case");
            let number = |key: &str| case[key].as_i64().expect("fixture number");
            let mut input = snapshot(location);
            input.documents.insert(
                "triggers".into(),
                json!({"minutely_orders": {"description": "Minutely orders", "job_key": "orders",
                    "starts_at": "2026-01-01T00:00:00Z",
                    "schedule": {"kind": "cron", "expression": case["expression"], "timezone": "UTC"}}}),
            );
            publish_snapshot(store, &input).await.expect("publication");
            let trigger_id = list_triggers(store, location, "development")
                .await
                .expect("trigger")[0]
                .trigger_id;
            // Publication records a sub-second start; place it a few minutes back so ticks are due.
            let start_minute = Utc::now()
                .duration_trunc(TimeDelta::minutes(1))
                .expect("minute")
                - TimeDelta::minutes(number("start_minutes_ago"));
            let started =
                start_minute + TimeDelta::microseconds(number("start_fraction_microseconds"));
            set_trigger_state(store, trigger_id,
                "UPDATE sqb_trigger_state SET started_at=$1, last_tick_for=NULL, next_due_at=NULL WHERE trigger_id=$2",
                started).await;
            let lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("election")
                .expect("lease");
            evaluate_trigger(store, &lease, trigger_id)
                .await
                .expect("first evaluation")
                .expect("trigger");
            let first = tick_times(store, trigger_id).await;
            assert_eq!(
                first.iter().min(),
                Some(
                    &(start_minute
                        + TimeDelta::minutes(test_case.expected_first_tick_minutes_after_start))
                ),
                "{}",
                test_case.description
            );
            // A cursor written by an earlier build sits just past its occurrence; resuming from it
            // must not tick that occurrence again at the whole second.
            let latest: DateTime<Utc> = *first.iter().max().expect("latest tick");
            let legacy =
                latest + TimeDelta::microseconds(number("legacy_cursor_fraction_microseconds"));
            set_trigger_state(
                store,
                trigger_id,
                "UPDATE sqb_trigger_state SET last_tick_for=$1 WHERE trigger_id=$2",
                legacy,
            )
            .await;
            evaluate_trigger(store, &lease, trigger_id)
                .await
                .expect("resumed evaluation")
                .expect("trigger");
            let resumed = tick_times(store, trigger_id).await;
            let seconds: BTreeSet<i64> = resumed.iter().map(DateTime::timestamp).collect();
            assert_eq!(
                resumed.len() - seconds.len(),
                test_case.expected_repeated_occurrences,
                "{}: {resumed:?}",
                test_case.description
            );
            assert_eq!(
                resumed
                    .iter()
                    .filter(|time| time.second() != 0 || time.nanosecond() != 0)
                    .count(),
                test_case.expected_fractional_ticks,
                "{}: {resumed:?}",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}
