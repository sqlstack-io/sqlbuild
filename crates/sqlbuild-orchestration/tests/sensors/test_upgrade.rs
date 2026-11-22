//! A snapshot published with an old-form sensor still runs its jobs after the upgrade drops the sensor.

use serde_json::json;
use uuid::Uuid;

use crate::credentials::models::DefaultRole;
use crate::credentials::tests::role_helpers::{credentials, role_fixture};
use crate::declarations::main::frozen_definitions::frozen_definitions;
use crate::declarations::main::read_person_job::read_person_job;
use crate::declarations::models::ServerPoolCaps;
use crate::runs::main::requests::request_job_run::request_job_run;
use crate::runs::models::{JobRunRequest, NodeOutcome, RunOutcome};
use crate::scheduler::tests::contracts::helpers::acquire_lease;
use crate::sensors::tests::helpers::{add_old_form_sensor, finish_run, triggers_document};
use crate::sensors::tests::test_types::SensorUpgradeTestCase;
use crate::snapshots::main::publication::publish_snapshot::publish_snapshot;
use crate::store::main::{
    begin::begin, commit::commit, migrate_store::migrate_store, query::query,
};
use crate::store::models::TransactionMode;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;
use crate::store_migrations::_helpers::migration::MIGRATIONS;
use crate::store_migrations::_helpers::migration_run::migrate_with;
use crate::store_migrations::_helpers::old_form_sensors::drop_old_form_sensors;

#[test]
fn given_a_snapshot_with_an_old_form_sensor_when_the_store_upgrades_then_its_jobs_queue_retry_run_as_a_group_and_show()
 {
    let test_cases = [
        SensorUpgradeTestCase {
            description: "SQLite",
            backend: Backend::Sqlite,
            expected_job_runs: 2,
        },
        SensorUpgradeTestCase {
            description: "PostgreSQL",
            backend: Backend::Postgres,
            expected_job_runs: 2,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_with(store, &MIGRATIONS[..3])
                .await
                .expect("a version 4 store");
            let roles = role_fixture(store).await;
            let location = roles.location_id;
            let mut input = crate::runs::tests::helpers::snapshot(location);
            input.generation = 2;
            input.documents.insert("jobs".into(), json!({
                "orders": {"description": "Refresh orders", "selection": ["customers", "orders"],
                    "on_failure": {"retries": 1, "retry_delay_seconds": 0}}
            }));
            input.documents.insert("groups".into(), json!({
                "nightly": {"description": "Orders nightly", "selection": ["customers", "orders"],
                    "jobs": {"orders": []}}
            }));
            input.documents.insert(
                "triggers".into(),
                json!({
                    "hourly_orders": {"description": "Refresh orders hourly", "job_key": "orders",
                        "schedule": {"kind": "cron", "expression": "0 * * * *", "timezone": "UTC"}}
                }),
            );
            let snapshot = publish_snapshot(store, &input)
                .await
                .expect("publish before upgrade");
            add_old_form_sensor(store, &snapshot).await;

            migrate_store(store).await.expect("upgrade");
            let migrated = triggers_document(store, snapshot.snapshot_id).await;
            migrate_store(store).await.expect("open again");
            let again =
                drop_old_form_sensors(begin(store, TransactionMode::Write).await.expect("step"))
                    .await
                    .expect("run the step again");
            commit(again).await.expect("commit step");

            let (transaction, definitions) = frozen_definitions(
                begin(store, TransactionMode::Read).await.expect("read"),
                snapshot.snapshot_id,
            )
            .await
            .expect("read the snapshot");
            let definitions = definitions.expect("the snapshot decodes");
            let (transaction, hidden) = query(
                transaction,
                "SELECT removed_in_snapshot_id FROM sqb_triggers WHERE name='health_orders'",
                Vec::new(),
            )
            .await
            .expect("old trigger row");
            commit(transaction).await.expect("commit");
            assert_eq!(
                (
                    definitions.sensors.len(),
                    definitions.triggers.keys().cloned().collect::<Vec<_>>(),
                    hidden.len(),
                    hidden[0]["removed_in_snapshot_id"].is_null(),
                    triggers_document(store, snapshot.snapshot_id).await,
                ),
                (0, vec!["hourly_orders".to_owned()], 1, false, migrated),
                "{}: the old sensor leaves the document, its row stays hidden, and opening again changes nothing",
                test_case.description
            );

            let lease = acquire_lease(store, Uuid::now_v7())
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
            let grouped = request_job_run(
                store,
                (&roles.context, None),
                &JobRunRequest {
                    run_id: Uuid::now_v7(),
                    location_id: location,
                    target: "development".into(),
                    job_key: "nightly".into(),
                    reason: "Run the group after the upgrade".into(),
                    priority: 0,
                    window: None,
                    full_refresh: false,
                    select: Vec::new(),
                    exclude: Vec::new(),
                },
            )
            .await
            .expect("queue the group");
            let (transaction, job_runs) = query(
                begin(store, TransactionMode::Read).await.expect("read"),
                "SELECT run_id FROM sqb_runs WHERE job_key='orders' AND parent_run_id IS NULL",
                vec![],
            )
            .await
            .expect("job runs");
            commit(transaction).await.expect("commit");
            let page = read_person_job(
                store,
                (&roles.context, credentials(&roles.administrator)),
                (location, "development", "orders"),
                (DefaultRole::None, &ServerPoolCaps::default()),
            )
            .await
            .expect("job page")
            .map(|job| job.job_key);
            assert_eq!(
                (job_runs.len(), grouped, page.as_deref()),
                (test_case.expected_job_runs, true, Some("orders")),
                "{}: the failed run is retried, the group queues and the job page loads",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}
