//! A store from before 0012 holds `partial` runs, facts and sensors; the upgrade makes them all `failed`.

use serde_json::json;
use uuid::Uuid;

use crate::credentials::tests::location_helpers::create_location;
use crate::declarations::main::frozen_definitions::frozen_definitions;
use crate::runs::models::{NodeOutcome, RunOutcome};
use crate::runs::tests::helpers::{json_column, snapshot};
use crate::scheduler::tests::contracts::helpers::acquire_lease;
use crate::sensors::models::{RunEventKind, SensorDefinition};
use crate::sensors::tests::helpers::{finish_run, watch_kinds, watched_run_kinds};
use crate::sensors::tests::test_types::PartialOutcomeUpgradeTestCase;
use crate::snapshots::main::publication::publish_snapshot::publish_snapshot;
use crate::store::main::migrate_store::migrate_store;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::TransactionMode;
use crate::store::tests::helpers::read_recall_events;
use crate::store::tests::schema::helpers::{
    POSTGRES_STORE, SQLITE_STORE, create_unversioned_baseline,
};
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store_migrations::_helpers::migration::MIGRATIONS;
use crate::store_migrations::_helpers::migration_run::migrate_with;

#[test]
fn given_a_store_with_partial_runs_and_sensors_when_upgraded_then_they_are_failed_runs_and_failed_watchers()
 {
    let test_cases = [
        PartialOutcomeUpgradeTestCase {
            description: "SQLite",
            dialect: &SQLITE_STORE,
            mark_partial: "UPDATE sqb_runs SET status='partial' WHERE job_key='orders'; \
                UPDATE recall_events SET payload=json_set(payload,'$.change.change.outcome','partial') \
                WHERE event_type='run_terminated'; \
                UPDATE recall_events SET payload=json_set(payload,'$.platform.outcome','partial') \
                WHERE event_type='platform_run_failed'",
            watched: &[
                ("orders_partial_sensor", &["partial"]),
                ("orders_failed_or_partial_sensor", &["failed", "partial"]),
                ("orders_success_sensor", &["success"]),
            ],
            expected_outcomes: ["failed"; 3],
            expected_watched: &[
                ("orders_failed_or_partial_sensor", &[RunEventKind::Failed]),
                ("orders_partial_sensor", &[RunEventKind::Failed]),
                ("orders_success_sensor", &[RunEventKind::Success]),
            ],
        },
        PartialOutcomeUpgradeTestCase {
            description: "PostgreSQL",
            dialect: &POSTGRES_STORE,
            mark_partial: "UPDATE sqb_runs SET status='partial' WHERE job_key='orders'; \
                UPDATE recall_events SET payload=jsonb_set(payload,'{change,change,outcome}','\"partial\"') \
                WHERE event_type='run_terminated'; \
                UPDATE recall_events SET payload=jsonb_set(payload,'{platform,outcome}','\"partial\"') \
                WHERE event_type='platform_run_failed'",
            watched: &[
                ("orders_partial_sensor", &["partial"]),
                ("orders_failed_or_partial_sensor", &["failed", "partial"]),
                ("orders_success_sensor", &["success"]),
            ],
            expected_outcomes: ["failed"; 3],
            expected_watched: &[
                ("orders_failed_or_partial_sensor", &[RunEventKind::Failed]),
                ("orders_partial_sensor", &[RunEventKind::Failed]),
                ("orders_success_sensor", &[RunEventKind::Success]),
            ],
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.dialect.backend).await else {
                return;
            };
            let store = &fixture.store;
            let before_0012 = MIGRATIONS
                .iter()
                .position(|migration| migration.name == "failed_run_outcome")
                .expect("migration 0012");
            create_unversioned_baseline(
                store,
                test_case.dialect,
                test_case.dialect.baseline_checksum,
            )
            .await;
            migrate_with(store, &MIGRATIONS[..before_0012])
                .await
                .expect("a store from before 0012");
            let location = create_location(store, "partial_upgrade", "test-admin")
                .await
                .expect("location");
            let mut input = snapshot(location);
            input.documents.insert(
                "jobs".into(),
                json!({"orders": {"description": "Refresh orders", "selection": ["customers", "orders"]}}),
            );
            let sensors: serde_json::Map<String, serde_json::Value> = test_case
                .watched
                .iter()
                .map(|(name, _)| {
                    ((*name).to_owned(), json!({
                        "description": "Watch orders", "function": "start_inventory", "code_hash": "v1",
                        "timeout_seconds": 60, "minimum_interval_seconds": 1,
                        "when": {"kind": "run_event", "job_key": "orders", "kinds": ["success"]}
                    }))
                })
                .collect();
            input.documents.insert("triggers".into(), json!(sensors));
            let published = publish_snapshot(store, &input)
                .await
                .expect("publish sensors");
            watch_kinds(store, &published, test_case.watched).await;
            let lease = acquire_lease(store, (Uuid::now_v7(), None))
                .await
                .expect("elect")
                .expect("leader");
            finish_run(
                store,
                &lease,
                (location, "development"),
                "orders",
                RunOutcome::Failed,
                [NodeOutcome::Success, NodeOutcome::Failed],
                [true, true],
            )
            .await;
            let mut transaction = begin(store, TransactionMode::Write).await.expect("mark");
            for statement in test_case.mark_partial.split(';') {
                (transaction, _) = query(transaction, statement, Vec::new())
                    .await
                    .expect("store the run as partial");
            }
            commit(transaction).await.expect("commit");

            migrate_store(store).await.expect("upgrade");

            let (transaction, runs) = query(
                begin(store, TransactionMode::Read).await.expect("read"),
                "SELECT status FROM sqb_runs WHERE job_key='orders'",
                Vec::new(),
            )
            .await
            .expect("runs");
            let (transaction, rows) = query(
                transaction,
                "SELECT name, definition_json FROM sqb_triggers WHERE kind='sensor' ORDER BY name",
                Vec::new(),
            )
            .await
            .expect("trigger rows");
            let (transaction, definitions) = frozen_definitions(transaction, published.snapshot_id)
                .await
                .expect("read the snapshot");
            commit(transaction).await.expect("commit");
            let events = read_recall_events(store, 0, 500).await.expect("facts");
            let outcome = |event_type: &str, pointer: &str| {
                events
                    .iter()
                    .find(|event| event["event_type"] == event_type)
                    .and_then(|event| event["payload"].pointer(pointer))
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned)
            };
            let in_rows: Vec<(String, Vec<RunEventKind>)> = rows
                .iter()
                .map(|row| {
                    let definition: SensorDefinition =
                        serde_json::from_value(json_column(&row["definition_json"]))
                            .expect("a trigger row decodes");
                    (
                        row["name"].as_str().unwrap_or_default().to_owned(),
                        watched_run_kinds(&definition),
                    )
                })
                .collect();
            let in_document: Vec<(String, Vec<RunEventKind>)> = definitions
                .expect("the snapshot decodes")
                .sensors
                .iter()
                .map(|(name, definition)| (name.clone(), watched_run_kinds(definition)))
                .collect();
            let expected: Vec<(String, Vec<RunEventKind>)> = test_case
                .expected_watched
                .iter()
                .map(|(name, kinds)| ((*name).to_owned(), kinds.to_vec()))
                .collect();
            assert_eq!(
                (
                    [
                        runs[0]["status"].as_str().map(str::to_owned),
                        outcome("run_terminated", "/change/change/outcome"),
                        outcome("platform_run_failed", "/platform/outcome"),
                    ],
                    in_document,
                    in_rows,
                ),
                (
                    test_case
                        .expected_outcomes
                        .map(|outcome| Some(outcome.to_owned())),
                    expected.clone(),
                    expected,
                ),
                "{}: the run, its facts and its watchers say failed",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}
