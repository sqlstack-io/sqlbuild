//! Migration 0004 drops the removed `resolved_concurrency.source` from stored job definitions, and a pinned snapshot document still in the old form fails.

use std::collections::BTreeMap;

use chrono::Utc;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::credentials::models::AuditContext;
use crate::credentials::tests::location_helpers::create_location;
use crate::runs::main::{
    execution::advance_groups::advance_groups, requests::request_job_run::request_job_run,
};
use crate::runs::models::JobRunRequest;
use crate::runs::tests::helpers::{json_column, rows, snapshot};
use crate::scheduler::main::_scheduler_cycle::scheduler_cycle;
use crate::scheduler::models::SchedulerState;
use crate::scheduler::tests::contracts::helpers::acquire_lease;
use crate::scheduler::tests::contracts::test_types::StoredConcurrencySourceTestCase;
use crate::scheduler::tests::contracts::ticks::helpers::snapshot as scheduled_snapshot;
use crate::sensors::tests::helpers::append_events;
use crate::snapshots::main::publication::publish_snapshot::publish_snapshot;
use crate::store::errors::ControlError;
use crate::store::main::compress::compress;
use crate::store::main::decompress::decompress;
use crate::store::main::query::query;
use crate::store::main::{begin::begin, commit::commit, migrate_store::migrate_store};
use crate::store::models::{ControlStore, Parameter, TransactionMode};
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;
use crate::store_migrations::_helpers::migration::MIGRATIONS;
use crate::store_migrations::_helpers::migration_run::migrate_with;

/// The field 0004 removes, as releases before it stored it.
const OLD_SOURCE: &str = "project";

const BACKENDS: [StoredConcurrencySourceTestCase; 2] = [
    StoredConcurrencySourceTestCase {
        description: "SQLite store",
        backend: Backend::Sqlite,
    },
    StoredConcurrencySourceTestCase {
        description: "PostgreSQL store",
        backend: Backend::Postgres,
    },
];

fn operator() -> AuditContext {
    AuditContext {
        actor: "test-operator".into(),
        granting_role: "control-store-owner".into(),
        request_id: Uuid::now_v7(),
        ip_address: None,
    }
}

fn manual_run(location_id: Uuid, job_key: &str) -> JobRunRequest {
    JobRunRequest {
        run_id: Uuid::now_v7(),
        location_id,
        target: "development".into(),
        job_key: job_key.into(),
        reason: "Build it now".into(),
        priority: 0,
        window: None,
        full_refresh: false,
        select: Vec::new(),
        exclude: Vec::new(),
    }
}

/// Write every published job's definition back with the source an older release recorded.
async fn store_old_job_definitions(store: &ControlStore) {
    let jobs = rows(
        store,
        "SELECT job_key, definition_json FROM sqb_jobs",
        vec![],
    )
    .await;
    let mut transaction = begin(store, TransactionMode::Write).await.expect("begin");
    for job in jobs {
        let mut definition: Value = json_column(&job["definition_json"]);
        definition["resolved_concurrency"]["source"] = json!(OLD_SOURCE);
        (transaction, _) = query(
            transaction,
            "UPDATE sqb_jobs SET definition_json=$1 WHERE job_key=$2",
            vec![
                Parameter::Json(definition),
                Parameter::Text(job["job_key"].as_str().expect("job key").to_owned()),
            ],
        )
        .await
        .expect("store the old definition");
    }
    commit(transaction).await.expect("commit");
}

/// Rewrite the stored `jobs` document of every snapshot as an older release published it.
async fn store_old_jobs_documents(store: &ControlStore) {
    let documents = rows(
        store,
        "SELECT b.document_hash, b.body FROM sqb_snapshot_documents d \
         JOIN sqb_documents b ON b.document_hash=d.document_hash WHERE d.kind='jobs'",
        vec![],
    )
    .await;
    let mut transaction = begin(store, TransactionMode::Write).await.expect("begin");
    for document in documents {
        let mut body: Value =
            serde_json::from_slice(&decompress(&document["body"]).expect("body")).expect("JSON");
        for job in body.as_object_mut().expect("jobs").values_mut() {
            job["resolved_concurrency"]["source"] = json!(OLD_SOURCE);
        }
        (transaction, _) = query(
            transaction,
            "UPDATE sqb_documents SET body=$1 WHERE document_hash=$2",
            vec![
                Parameter::Bytes(
                    compress(&serde_json::to_vec(&body).expect("JSON")).expect("gzip"),
                ),
                Parameter::Text(document["document_hash"].as_str().expect("hash").to_owned()),
            ],
        )
        .await
        .expect("store the old document");
    }
    commit(transaction).await.expect("commit");
}

#[test]
fn given_a_job_stored_with_its_concurrency_source_when_migrating_then_schedules_sensors_and_manual_runs_read_it()
 {
    let mut completed = 0;
    for test_case in BACKENDS {
        run(async {
            let fixture = open_test_store(test_case.backend)
                .await
                .expect("required backend");
            let store = &fixture.store;
            migrate_with(store, &MIGRATIONS[..2])
                .await
                .expect("a store at version 3");
            let location = create_location(store, "orders", "test-admin")
                .await
                .expect("location");
            let mut input = scheduled_snapshot(location);
            input.documents.insert(
                "jobs".into(),
                json!({"orders": {"description": "Build orders", "selection": ["orders"],
                    "resolved_concurrency": {"limit": 4}}}),
            );
            input.documents.get_mut("triggers").expect("triggers")["health_orders"] = json!({
                "description": "Refresh orders on overdue ticks", "job_key": "orders",
                "when": {"kind": "platform_event", "kinds": ["tick_overdue"]},
                "minimum_interval_seconds": 1
            });
            publish_snapshot(store, &input).await.expect("publish");
            store_old_job_definitions(store).await;
            let before =
                request_job_run(store, (&operator(), None), &manual_run(location, "orders"))
                    .await
                    .map(|_| ());

            migrate_store(store).await.expect("migrate to version 4");
            let stored = rows(store, "SELECT definition_json FROM sqb_jobs", vec![]).await;
            append_events(store, 0..1, Utc::now()).await;
            let (_, cycle) = scheduler_cycle(store, SchedulerState::new(Uuid::now_v7()))
                .await
                .expect("a scheduler cycle over the migrated jobs");
            request_job_run(store, (&operator(), None), &manual_run(location, "orders"))
                .await
                .expect("a manual run of the migrated job");
            let runs = rows(
                store,
                "SELECT origin, COUNT(*) AS runs FROM sqb_runs GROUP BY origin ORDER BY origin",
                vec![],
            )
            .await;

            assert_eq!(
                (
                    matches!(&before, Err(ControlError::Invalid(message)) if message.contains("unknown field `source`")),
                    json_column(&stored[0]["definition_json"])["resolved_concurrency"].clone(),
                    cycle.ticks,
                    runs.iter()
                        .map(|row| format!(
                            "{} {}",
                            row["origin"].as_str().unwrap_or_default(),
                            row["runs"]
                        ))
                        .collect::<Vec<_>>(),
                ),
                (
                    true,
                    json!({"limit": 4}),
                    1,
                    vec![
                        "manual 1".to_owned(),
                        "schedule 1".to_owned(),
                        "sensor 1".to_owned()
                    ],
                ),
                "{}: refused before 0004, then read by the scheduler, its sensor and a manual run",
                test_case.description
            );
            close_test_store(fixture).await;
            completed += 1;
        });
    }
    assert_eq!(completed, BACKENDS.len(), "every backend ran");
}

// Runs pinned to a snapshot published before 0004 read its immutable documents, which 0004 can't
// rewrite. They still fail with `ControlError::Invalid`, which stops the scheduler; operators let
// such runs finish or cancel them before upgrading. Turning this into a per-run failure is tracked
// separately, and that change replaces this expectation.
#[test]
fn given_a_group_pinned_to_a_snapshot_document_in_the_old_form_when_advancing_it_then_it_fails_invalid()
 {
    let mut completed = 0;
    for test_case in BACKENDS {
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
            input.nodes.get_mut("orders").expect("orders").definition["deps"] =
                json!(["customers"]);
            input.documents.insert(
                "jobs".into(),
                json!({
                    "staging_job": {"description": "Stage customers", "selection": ["customers"],
                        "resolved_concurrency": {"limit": 4}},
                    "marts_job": {"description": "Build orders", "selection": ["orders"],
                        "resolved_concurrency": {"limit": 4}}
                }),
            );
            input.documents.insert(
                "groups".into(),
                json!({"nightly_job_group": {
                    "description": "Stage then build orders", "selection": ["customers", "orders"],
                    "jobs": {"staging_job": [], "marts_job": ["staging_job"]}
                }}),
            );
            publish_snapshot(store, &input).await.expect("publish");
            request_job_run(
                store,
                (&operator(), None),
                &manual_run(location, "nightly_job_group"),
            )
            .await
            .expect("request the group");
            let lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("elect")
                .expect("leader");
            let limits = BTreeMap::new();
            let scope = (location, "development");
            advance_groups(store, &lease, (&limits, scope))
                .await
                .expect("start the group");
            store_old_jobs_documents(store).await;

            let advanced = advance_groups(store, &lease, (&limits, scope)).await;

            assert!(
                matches!(&advanced, Err(ControlError::Invalid(message)) if message == "jobs: unknown field `source`, expected `limit`"),
                "{}: {advanced:?}",
                test_case.description
            );
            close_test_store(fixture).await;
            completed += 1;
        });
    }
    assert_eq!(completed, BACKENDS.len(), "every backend ran");
}
