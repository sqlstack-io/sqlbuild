//! Migration 0004 drops the removed `resolved_concurrency.source` from stored job definitions, and a pinned snapshot document still in the old form fails.

use std::collections::BTreeMap;

use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

use crate::credentials::tests::location_helpers::create_location;
use crate::runs::main::{
    execution::advance_groups::advance_groups, requests::request_job_run::request_job_run,
};
use crate::runs::tests::helpers::{json_column, rows, snapshot};
use crate::scheduler::models::SchedulerState;
use crate::scheduler::tests::contracts::helpers::{
    acquire_lease, manual_run, operator, scheduler_cycle, store_old_job_definitions,
    store_old_jobs_documents,
};
use crate::scheduler::tests::contracts::test_types::{
    MigratedConcurrencySourceTestCase, PinnedOldDocumentTestCase,
};
use crate::scheduler::tests::contracts::ticks::helpers::snapshot as scheduled_snapshot;
use crate::sensors::tests::helpers::append_events;
use crate::snapshots::main::publication::publish_snapshot::publish_snapshot;
use crate::store::errors::ControlError;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;
use crate::store_migrations::_helpers::migration::MIGRATIONS;
use crate::store_migrations::_helpers::migration_run::migrate_with;

#[test]
fn given_a_job_stored_with_its_concurrency_source_when_migrating_then_schedules_sensors_and_manual_runs_read_it()
 {
    let test_cases = [
        MigratedConcurrencySourceTestCase {
            description: "SQLite store",
            backend: Backend::Sqlite,
            stored_source: "project",
            expected_refused_before: true,
            expected_resolved_concurrency: json!({"limit": 4}),
            expected_ticks: 1,
            expected_runs: &["manual 1", "schedule 1", "sensor 1"],
        },
        MigratedConcurrencySourceTestCase {
            description: "PostgreSQL store",
            backend: Backend::Postgres,
            stored_source: "target",
            expected_refused_before: true,
            expected_resolved_concurrency: json!({"limit": 4}),
            expected_ticks: 1,
            expected_runs: &["manual 1", "schedule 1", "sensor 1"],
        },
    ];
    for test_case in test_cases {
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
            store_old_job_definitions(store, test_case.stored_source).await;
            let before =
                request_job_run(store, (&operator(), None), &manual_run(location, "orders")).await;

            migrate_store(store).await.expect("migrate to version 4");
            let stored = rows(store, "SELECT definition_json FROM sqb_jobs", vec![]).await;
            append_events(store, 0..1, Utc::now()).await;
            let (_, cycle) = scheduler_cycle(store, SchedulerState::new(Uuid::now_v7()))
                .await
                .expect("a scheduler cycle over the migrated jobs");
            request_job_run(store, (&operator(), None), &manual_run(location, "orders"))
                .await
                .expect("a manual run of the migrated job");
            let runs: Vec<String> = rows(
                store,
                "SELECT origin, COUNT(*) AS runs FROM sqb_runs GROUP BY origin ORDER BY origin",
                vec![],
            )
            .await
            .iter()
            .map(|row| {
                format!(
                    "{} {}",
                    row["origin"].as_str().unwrap_or_default(),
                    row["runs"]
                )
            })
            .collect();

            assert_eq!(
                (
                    matches!(&before, Err(ControlError::Invalid(message)) if message.contains("unknown field `source`")),
                    json_column(&stored[0]["definition_json"])["resolved_concurrency"].clone(),
                    cycle.ticks,
                    runs,
                ),
                (
                    test_case.expected_refused_before,
                    test_case.expected_resolved_concurrency.clone(),
                    test_case.expected_ticks,
                    test_case
                        .expected_runs
                        .iter()
                        .map(|run| (*run).to_owned())
                        .collect::<Vec<_>>(),
                ),
                "{}: refused before 0004, then read by the scheduler, its sensor and a manual run",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}

// Runs pinned to a snapshot published before 0004 read its immutable documents, which 0004 can't
// rewrite. They still fail with `ControlError::Invalid`, which stops the scheduler; operators let
// such runs finish or cancel them before upgrading. Turning this into a per-run failure is tracked
// separately, and that change replaces this expectation.
#[test]
fn given_a_group_pinned_to_a_snapshot_document_in_the_old_form_when_advancing_it_then_it_fails_invalid()
 {
    let test_cases = [
        PinnedOldDocumentTestCase {
            description: "SQLite store",
            backend: Backend::Sqlite,
            stored_source: "project",
            expected_error: "jobs: unknown field `source`, expected `limit`",
        },
        PinnedOldDocumentTestCase {
            description: "PostgreSQL store",
            backend: Backend::Postgres,
            stored_source: "default",
            expected_error: "jobs: unknown field `source`, expected `limit`",
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
            store_old_jobs_documents(store, test_case.stored_source).await;

            let advanced = advance_groups(store, &lease, (&limits, scope)).await;

            assert_eq!(
                advanced.map_err(|error| match error {
                    ControlError::Invalid(message) => message,
                    other => format!("not Invalid: {other:?}"),
                }),
                Err(test_case.expected_error.to_owned()),
                "{}: the group pinned to the old document fails Invalid",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}
