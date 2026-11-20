//! Migration 0004 drops the removed `resolved_concurrency.source` from stored job definitions; until a location publishes again, reads of its snapshot documents still fail.

use std::collections::BTreeMap;

use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

use crate::credentials::models::DefaultRole;
use crate::credentials::tests::location_helpers::create_location;
use crate::credentials::tests::role_helpers::{credentials, role_fixture};
use crate::declarations::main::read_person_job::read_person_job;
use crate::declarations::models::ServerPoolCaps;
use crate::runs::main::{
    execution::advance_groups::advance_groups, requests::request_job_run::request_job_run,
};
use crate::runs::tests::helpers::{json_column, rows, snapshot};
use crate::scheduler::models::SchedulerState;
use crate::scheduler::tests::contracts::helpers::{
    acquire_lease, manual_run, operator, outcome, scheduler_cycle, store_old_documents,
    store_old_job_definitions, upgrade_snapshot,
};
use crate::scheduler::tests::contracts::test_types::{
    GroupScheduleOldDocumentTestCase, MigratedConcurrencySourceTestCase, PinnedOldDocumentTestCase,
};
use crate::scheduler::tests::contracts::ticks::helpers::calendar_text;
use crate::sensors::tests::helpers::append_events;
use crate::snapshots::main::publication::publish_snapshot::publish_snapshot;
use crate::store::errors::ControlError;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::lease_helpers::expire_lease;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;
use crate::store_migrations::_helpers::migration::MIGRATIONS;
use crate::store_migrations::_helpers::migration_run::migrate_with;

/// How reading a document an older release published, with `source`, fails.
const OLD_FORM: &str = "Invalid(\"jobs: unknown field `source`, expected `limit`\")";

#[test]
fn given_jobs_published_before_0004_when_migrating_then_plain_job_runs_work_and_snapshot_reads_wait_for_a_republish()
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
            expected_until_republished: [OLD_FORM, OLD_FORM],
            expected_after_republish: ["ok", "ok"],
        },
        MigratedConcurrencySourceTestCase {
            description: "PostgreSQL store",
            backend: Backend::Postgres,
            stored_source: "target",
            expected_refused_before: true,
            expected_resolved_concurrency: json!({"limit": 4}),
            expected_ticks: 1,
            expected_runs: &["manual 1", "schedule 1", "sensor 1"],
            expected_until_republished: [OLD_FORM, OLD_FORM],
            expected_after_republish: ["ok", "ok"],
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
            let roles = role_fixture(store).await;
            let location = roles.location_id;
            let mut input = upgrade_snapshot(
                location,
                json!({
                    "daily_orders": {"description": "Refresh orders", "job_key": "orders",
                        "starts_at": calendar_text(0),
                        "schedule": {"kind": "calendar", "times": [calendar_text(0), calendar_text(1),
                            calendar_text(2), "2030-01-01T00:00:00Z"], "refresh_at": "2029-01-01T00:00:00Z"}},
                    "health_orders": {"description": "Refresh orders on overdue ticks", "job_key": "orders",
                        "when": {"kind": "platform_event", "kinds": ["tick_overdue"]},
                        "minimum_interval_seconds": 1}
                }),
            );
            input.generation = 2;
            publish_snapshot(store, &input).await.expect("publish");
            store_old_job_definitions(store, test_case.stored_source).await;
            store_old_documents(store, test_case.stored_source).await;
            let before =
                request_job_run(store, (&operator(), None), &manual_run(location, "orders")).await;
            let caps = ServerPoolCaps::default();
            let administrator = (&roles.context, credentials(&roles.administrator));
            let page_and_group = || async {
                [
                    outcome(
                        read_person_job(
                            store,
                            administrator,
                            (location, "development", "orders"),
                            (DefaultRole::None, &caps),
                        )
                        .await,
                    ),
                    outcome(
                        request_job_run(
                            store,
                            (&operator(), None),
                            &manual_run(location, "nightly_group"),
                        )
                        .await,
                    ),
                ]
            };

            migrate_store(store).await.expect("migrate to version 4");
            let stored = rows(
                store,
                "SELECT definition_json FROM sqb_jobs WHERE job_key='orders'",
                vec![],
            )
            .await;
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
            let until_republished = page_and_group().await;
            input.generation = 3;
            publish_snapshot(store, &input)
                .await
                .expect("the location publishes again");
            let after_republish = page_and_group().await;

            assert_eq!(
                (
                    matches!(&before, Err(ControlError::Invalid(message)) if message.contains("unknown field `source`")),
                    json_column(&stored[0]["definition_json"])["resolved_concurrency"].clone(),
                    cycle.ticks,
                    runs,
                    until_republished,
                    after_republish,
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
                    test_case.expected_until_republished.map(str::to_owned),
                    test_case.expected_after_republish.map(str::to_owned),
                ),
                "{}: refused before 0004; plain job runs after it; the job page and a group run once the location publishes again",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}

// Until its location publishes again, a schedule of a group reads the current snapshot's groups
// document, which 0004 can't rewrite, and its `ControlError::Invalid` fails the whole scheduler
// cycle. Making this a per-run failure is tracked separately, and that change replaces this
// expectation.
#[test]
fn given_a_group_schedule_published_before_0004_when_the_scheduler_cycles_then_it_fails_until_the_location_publishes_again()
 {
    let test_cases = [
        GroupScheduleOldDocumentTestCase {
            description: "SQLite store",
            backend: Backend::Sqlite,
            stored_source: "project",
            expected_until_republished: OLD_FORM,
            expected_ticks_after_republish: 1,
        },
        GroupScheduleOldDocumentTestCase {
            description: "PostgreSQL store",
            backend: Backend::Postgres,
            stored_source: "local",
            expected_until_republished: OLD_FORM,
            expected_ticks_after_republish: 1,
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
            let mut input = upgrade_snapshot(
                location,
                json!({"nightly_orders": {"description": "Run the group", "job_key": "nightly_group",
                    "starts_at": calendar_text(0),
                    "schedule": {"kind": "calendar", "times": [calendar_text(0), calendar_text(1),
                        calendar_text(2), "2030-01-01T00:00:00Z"], "refresh_at": "2029-01-01T00:00:00Z"}}}),
            );
            publish_snapshot(store, &input).await.expect("publish");
            store_old_job_definitions(store, test_case.stored_source).await;
            store_old_documents(store, test_case.stored_source).await;
            migrate_store(store).await.expect("migrate to version 4");

            let stopped = scheduler_cycle(store, SchedulerState::new(Uuid::now_v7())).await;
            input.generation = 2;
            publish_snapshot(store, &input)
                .await
                .expect("the location publishes again");
            // The stopped scheduler's lease runs out before a restarted one takes over.
            expire_lease(store).await;
            let (_, cycle) = scheduler_cycle(store, SchedulerState::new(Uuid::now_v7()))
                .await
                .expect("a scheduler cycle once the location has published again");

            assert_eq!(
                (outcome(stopped), cycle.ticks),
                (
                    test_case.expected_until_republished.to_owned(),
                    test_case.expected_ticks_after_republish
                ),
                "{}: the cycle fails Invalid until the location publishes again, then ticks",
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
            expected_error: OLD_FORM,
        },
        PinnedOldDocumentTestCase {
            description: "PostgreSQL store",
            backend: Backend::Postgres,
            stored_source: "default",
            expected_error: OLD_FORM,
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
            store_old_documents(store, test_case.stored_source).await;

            let advanced = advance_groups(store, &lease, (&limits, scope)).await;

            assert_eq!(
                outcome(advanced),
                test_case.expected_error,
                "{}: the group pinned to the old document fails Invalid",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}
