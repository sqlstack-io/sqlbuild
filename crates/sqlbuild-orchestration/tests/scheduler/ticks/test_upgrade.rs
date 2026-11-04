//! A store written by the pre-progress build upgrades in place and its scheduler resumes exactly once.

use std::collections::BTreeSet;

use chrono::{DateTime, DurationRound, TimeDelta, Utc};
use serde_json::json;
use uuid::Uuid;

use crate::credentials::tests::location_helpers::create_location;
use crate::declarations::main::list_triggers::list_triggers;
use crate::platform::main::rebuild_projections::rebuild_projections;
use crate::runs::main::list_runs::list_runs;
use crate::scheduler::main::{
    _acquire_lease::acquire_lease, _evaluate_trigger::evaluate_trigger,
    _scheduler_cycle::scheduler_cycle, list_ticks::list_ticks,
};
use crate::scheduler::models::SchedulerState;
use crate::scheduler::tests::contracts::ticks::helpers::{set_trigger_state, snapshot};
use crate::scheduler::tests::contracts::ticks::test_types::SchedulerUpgradeTestCase;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::store::errors::ControlError;
use crate::store::main::{
    begin::begin, commit::commit, migrate_store::migrate_store, query::query,
};
use crate::store::models::{Parameter, TransactionMode};
use crate::store::tests::lease_helpers::expire_lease;
use crate::store::tests::store_helpers::{
    close_test_store, open_test_store, reconstruct_fingerprint_predecessor, run,
};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_a_store_left_by_the_pre_progress_build_when_upgraded_then_the_scheduler_waits_for_the_old_lease_and_resumes_every_minute_once()
 {
    let test_cases = [
        SchedulerUpgradeTestCase {
            description: "SQLite in-place scheduler upgrade",
            backend: Backend::Sqlite,
            expected_follower_ticks: 0,
            expected_duplicate_minutes: 0,
            expected_missing_minutes: 0,
            expected_runs_per_tick: 1,
        },
        SchedulerUpgradeTestCase {
            description: "PostgreSQL in-place scheduler upgrade",
            backend: Backend::Postgres,
            expected_follower_ticks: 0,
            expected_duplicate_minutes: 0,
            expected_missing_minutes: 0,
            expected_runs_per_tick: 1,
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
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../benchmarks/duckshop/edge_cases/orchestration/scheduler_upgrade.json");
            let case: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(path).expect("upgrade fixture"))
                    .expect("upgrade case");
            let number = |key: &str| case[key].as_i64().expect("fixture number");
            let mut input = snapshot(location);
            input.documents.insert(
                "jobs".into(),
                json!({"orders": {"description": "Build orders", "selection": ["orders"], "overlap": "queue"}}),
            );
            input.documents.insert(
                "triggers".into(),
                json!({"minutely_orders": {"description": "Minutely orders", "job_key": "orders",
                    "starts_at": "2026-01-01T00:00:00Z", "catch_up": {"all": {"max_runs": number("old_build_ticks")}},
                    "schedule": {"kind": "cron", "expression": "* * * * *", "timezone": "UTC"}}}),
            );
            publish_snapshot(store, &input).await.expect("publication");
            let trigger_id = list_triggers(store, location, "development")
                .await
                .expect("trigger")[0]
                .trigger_id;
            let start = Utc::now()
                .duration_trunc(TimeDelta::minutes(1))
                .expect("minute")
                - TimeDelta::minutes(number("backlog_minutes"));
            set_trigger_state(store, trigger_id,
                "UPDATE sqb_trigger_state SET started_at=$1, last_tick_for=NULL, next_due_at=$1 WHERE trigger_id=$2",
                start).await;
            // The old build ticks one bounded batch, then the server stops for the upgrade.
            let old = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("old election")
                .expect("old lease");
            evaluate_trigger(store, &old, trigger_id)
                .await
                .expect("old build ticks")
                .expect("trigger");
            // That build ticked and recorded cursors a fraction past each whole minute.
            let fraction = TimeDelta::microseconds(number("old_build_fraction_microseconds"));
            let old_ticks = list_ticks(store, trigger_id, 1000)
                .await
                .expect("old ticks");
            let mut transaction = begin(store, TransactionMode::Write)
                .await
                .expect("old precision");
            for tick in &old_ticks {
                (transaction, _) = query(
                    transaction,
                    "UPDATE sqb_ticks SET scheduled_for=$1 WHERE tick_id=$2",
                    vec![
                        Parameter::Time(tick.scheduled_for + fraction),
                        Parameter::Id(tick.tick_id),
                    ],
                )
                .await
                .expect("old tick");
            }
            let old_cursor = old_ticks
                .iter()
                .map(|tick| tick.scheduled_for)
                .max()
                .expect("old cursor")
                + fraction;
            (transaction, _) = query(
                transaction,
                "UPDATE sqb_trigger_state SET last_tick_for=$1 WHERE trigger_id=$2",
                vec![Parameter::Time(old_cursor), Parameter::Id(trigger_id)],
            )
            .await
            .expect("old cursor");
            commit(transaction).await.expect("commit old precision");
            reconstruct_fingerprint_predecessor(store).await;
            migrate_store(store).await.expect("in-place upgrade");
            // The old holder's lease is still live: the upgraded scheduler must not take over.
            let (upgraded, follower) = scheduler_cycle(store, SchedulerState::new(Uuid::now_v7()))
                .await
                .expect("follower cycle");
            assert!(!follower.leader, "{}", test_case.description);
            assert_eq!(
                follower.ticks, test_case.expected_follower_ticks,
                "{}",
                test_case.description
            );
            expire_lease(store).await;
            let mut state = upgraded;
            for _ in 0..usize::try_from(number("backlog_minutes")).expect("minutes") {
                state = scheduler_cycle(store, state).await.expect("resume").0;
            }
            assert!(
                matches!(
                    evaluate_trigger(store, &old, trigger_id).await,
                    Err(ControlError::Fenced)
                ),
                "{}: the old build is fenced after takeover",
                test_case.description
            );
            let settled = Utc::now()
                .duration_trunc(TimeDelta::minutes(1))
                .expect("minute");
            let ticks = list_ticks(store, trigger_id, 1000).await.expect("ticks");
            let minutes: Vec<DateTime<Utc>> = ticks
                .iter()
                .map(|tick| {
                    tick.scheduled_for
                        .duration_trunc(TimeDelta::seconds(1))
                        .expect("second")
                })
                .collect();
            let distinct: BTreeSet<DateTime<Utc>> = minutes.iter().copied().collect();
            let expected: BTreeSet<DateTime<Utc>> = (0..)
                .map(|offset| start + TimeDelta::minutes(offset))
                .take_while(|minute| *minute <= settled - TimeDelta::minutes(1))
                .collect();
            assert_eq!(
                minutes.len() - distinct.len(),
                test_case.expected_duplicate_minutes,
                "{}: {minutes:?}",
                test_case.description
            );
            assert_eq!(
                expected.difference(&distinct).count(),
                test_case.expected_missing_minutes,
                "{}: {minutes:?}",
                test_case.description
            );
            let runs = list_runs(store, (location, "development"), 1000)
                .await
                .expect("runs");
            assert_eq!(
                runs.len(),
                ticks.len() * test_case.expected_runs_per_tick,
                "{}",
                test_case.description
            );
            // The upgraded scheduler recorded progress, so an operator repair now succeeds.
            let cursor = list_triggers(store, location, "development")
                .await
                .expect("cursor")[0]
                .last_tick_for;
            rebuild_projections(store)
                .await
                .expect("repair after upgrade");
            assert_eq!(
                list_triggers(store, location, "development")
                    .await
                    .expect("repaired")[0]
                    .last_tick_for,
                cursor,
                "{}",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}
