//! Migration 0004 drops the removed `resolved_concurrency.source` from stored job definitions; until a location publishes again, reads of its snapshot documents fail, and each fails only the run, trigger or request that reads them.

use std::collections::BTreeMap;

use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

use crate::credentials::models::{AuditContext, DefaultRole};
use crate::credentials::tests::location_helpers::create_location;
use crate::credentials::tests::role_helpers::{credentials, role_fixture};
use crate::declarations::main::read_person_job::read_person_job;
use crate::declarations::models::ServerPoolCaps;
use crate::runs::main::{
    execution::advance_groups::advance_groups, execution::claim_next_run::claim_next_run,
    requests::check_adhoc_run::check_adhoc_run, requests::request_job_run::request_job_run,
};
use crate::runs::models::{AdhocCheckRequest, NodeOutcome, RunChange, RunOutcome};
use crate::runs::tests::helpers::{adhoc_fixture, json_column, rows};
use crate::runs::tests::test_types::ExactKeyPlanner;
use crate::scheduler::models::SchedulerState;
use crate::scheduler::tests::contracts::helpers::{
    acquire_lease, group_snapshot, manual_run, operator, outcome, refusal, report_changes,
    report_run, run_states, scheduler_cycle, skipped_triggers, store_old_documents,
    store_old_job_definitions, store_unreadable_jobs, upgrade_snapshot,
};
use crate::scheduler::tests::contracts::test_types::{
    AdhocOldDocumentTestCase, ClaimOldDocumentTestCase, FailedJobRunOldDocumentTestCase,
    GroupScheduleOldDocumentTestCase, MigratedConcurrencySourceTestCase, PinnedOldDocumentTestCase,
    RunningMemberOldDocumentTestCase,
};
use crate::scheduler::tests::contracts::ticks::helpers::calendar_text;
use crate::sensors::tests::helpers::append_events;
use crate::snapshots::main::publication::publish_snapshot::publish_snapshot;
use crate::store::constants::UNREADABLE_PUBLICATION;
use crate::store::errors::ControlError;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;
use crate::store_migrations::_helpers::migration::MIGRATIONS;
use crate::store_migrations::_helpers::migration_run::migrate_with;

/// The schedule of a group whose snapshot can't be read, as the live problems list it.
const SKIPPED: &str = "nightly_orders: this trigger's job was published by an earlier SQLBuild release and can't be read; it runs again after its location publishes again";

#[test]
fn given_jobs_published_before_0004_when_migrating_then_plain_job_runs_work_and_snapshot_reads_fail_alone_until_a_republish()
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
            expected_until_republished: [UNREADABLE_PUBLICATION, UNREADABLE_PUBLICATION],
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
            expected_until_republished: [UNREADABLE_PUBLICATION, UNREADABLE_PUBLICATION],
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
                    refusal(
                        read_person_job(
                            store,
                            administrator,
                            (location, "development", "orders"),
                            (DefaultRole::None, &caps),
                        )
                        .await,
                    ),
                    refusal(
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
                    matches!(&before, Err(ControlError::UnreadableSnapshot { detail, .. }) if detail.contains("unknown field `source`")),
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
                "{}: refused before 0004; plain job runs after it; the job page and a group run fail on their own until the location publishes again",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_a_group_schedule_published_before_0004_when_the_scheduler_cycles_then_only_that_trigger_is_skipped_until_its_location_publishes_again()
 {
    let test_cases = [
        GroupScheduleOldDocumentTestCase {
            description: "SQLite store",
            backend: Backend::Sqlite,
            stored_source: "project",
            expected_cycle: "ok",
            expected_ticks: 1,
            expected_skipped: &[SKIPPED],
            expected_ticks_after_republish: 1,
        },
        GroupScheduleOldDocumentTestCase {
            description: "PostgreSQL store",
            backend: Backend::Postgres,
            stored_source: "local",
            expected_cycle: "ok",
            expected_ticks: 1,
            expected_skipped: &[SKIPPED],
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
            let roles = role_fixture(store).await;
            let location = roles.location_id;
            let schedule = |name: &str, job: &str| {
                json!({name: {"description": "Run it nightly", "job_key": job,
                    "starts_at": calendar_text(0),
                    "schedule": {"kind": "calendar", "times": [calendar_text(0), calendar_text(1),
                        calendar_text(2), "2030-01-01T00:00:00Z"], "refresh_at": "2029-01-01T00:00:00Z"}}})
            };
            let mut input = upgrade_snapshot(location, schedule("nightly_orders", "nightly_group"));
            input.generation = 2;
            publish_snapshot(store, &input).await.expect("publish");
            store_old_job_definitions(store, test_case.stored_source).await;
            store_old_documents(store, test_case.stored_source).await;
            migrate_store(store).await.expect("migrate to version 4");
            let other = create_location(store, "returns", "test-admin")
                .await
                .expect("another location");
            publish_snapshot(
                store,
                &upgrade_snapshot(other, schedule("nightly_returns", "orders")),
            )
            .await
            .expect("the other location publishes");

            let cycled = scheduler_cycle(store, SchedulerState::new(Uuid::now_v7())).await;
            let skipped = skipped_triggers(store, credentials(&roles.administrator)).await;
            input.generation = 3;
            publish_snapshot(store, &input)
                .await
                .expect("the location publishes again");
            let (state, cycle) = cycled.map_or_else(
                |_| (SchedulerState::new(Uuid::now_v7()), None),
                |(state, cycle)| (state, Some(cycle)),
            );
            let (_, after) = scheduler_cycle(store, state)
                .await
                .expect("a scheduler cycle once the location has published again");
            let cleared = skipped_triggers(store, credentials(&roles.administrator)).await;

            assert_eq!(
                (
                    cycle.as_ref().map_or("failed", |_| "ok"),
                    cycle.map(|cycle| cycle.ticks),
                    skipped,
                    after.ticks,
                    cleared,
                ),
                (
                    test_case.expected_cycle,
                    Some(test_case.expected_ticks),
                    test_case
                        .expected_skipped
                        .iter()
                        .map(|line| (*line).to_owned())
                        .collect::<Vec<_>>(),
                    test_case.expected_ticks_after_republish,
                    Vec::<String>::new(),
                ),
                "{}: the cycle carries on and the other location ticks; the group's schedule is listed as skipped until the location publishes again, then ticks",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_a_job_published_before_0004_when_its_run_fails_before_a_republish_then_it_fails_without_a_retry()
 {
    let test_cases = [
        FailedJobRunOldDocumentTestCase {
            description: "SQLite store",
            backend: Backend::Sqlite,
            stored_source: "project",
            expected_reports: ["ok", "ok", "ok"],
            expected_runs: &[
                "orders failed: the worker finished; not retried: this run's snapshot was published by an earlier SQLBuild release and can't be read; rerun it after its location publishes again",
            ],
        },
        FailedJobRunOldDocumentTestCase {
            description: "PostgreSQL store",
            backend: Backend::Postgres,
            stored_source: "default",
            expected_reports: ["ok", "ok", "ok"],
            expected_runs: &[
                "orders failed: the worker finished; not retried: this run's snapshot was published by an earlier SQLBuild release and can't be read; rerun it after its location publishes again",
            ],
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
            let mut input = upgrade_snapshot(location, json!({}));
            input.documents.insert(
                "jobs".into(),
                json!({"orders": {"description": "Build orders", "selection": ["orders"],
                    "resolved_concurrency": {"limit": 4},
                    "on_failure": {"retries": 2, "retry_delay_seconds": 0}}}),
            );
            publish_snapshot(store, &input).await.expect("publish");
            store_old_job_definitions(store, test_case.stored_source).await;
            store_old_documents(store, test_case.stored_source).await;
            migrate_store(store).await.expect("migrate to version 4");
            request_job_run(store, (&operator(), None), &manual_run(location, "orders"))
                .await
                .expect("a manual run of the migrated job");
            let lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("elect")
                .expect("leader");
            let claim = claim_next_run(
                store,
                &lease,
                &BTreeMap::new(),
                Some((location, "development")),
            )
            .await
            .expect("claim")
            .expect("the queued run");

            let reports =
                report_run(store, &claim, (NodeOutcome::Failed, RunOutcome::Failed)).await;

            assert_eq!(
                (reports, run_states(store).await),
                (
                    test_case.expected_reports.map(str::to_owned).to_vec(),
                    test_case
                        .expected_runs
                        .iter()
                        .map(|run| (*run).to_owned())
                        .collect::<Vec<_>>()
                ),
                "{}: the failure is recorded, and the retry it can't read the policy of is not queued",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_a_group_pinned_to_a_snapshot_document_in_the_old_form_when_advancing_it_then_only_that_group_fails()
 {
    let test_cases = [
        PinnedOldDocumentTestCase {
            description: "SQLite store",
            backend: Backend::Sqlite,
            stored_source: "project",
            expected_advance: "ok",
            expected_runs: &[
                "marts_job cancelled: cancelled before launch",
                "nightly_job_group failed: this run's snapshot was published by an earlier SQLBuild release and can't be read; rerun it after its location publishes again",
                "staging_job cancelled: cancelled before launch",
            ],
        },
        PinnedOldDocumentTestCase {
            description: "PostgreSQL store",
            backend: Backend::Postgres,
            stored_source: "default",
            expected_advance: "ok",
            expected_runs: &[
                "marts_job cancelled: cancelled before launch",
                "nightly_job_group failed: this run's snapshot was published by an earlier SQLBuild release and can't be read; rerun it after its location publishes again",
                "staging_job cancelled: cancelled before launch",
            ],
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
            publish_snapshot(store, &group_snapshot(location))
                .await
                .expect("publish");
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
                (outcome(advanced), run_states(store).await),
                (
                    test_case.expected_advance.to_owned(),
                    test_case
                        .expected_runs
                        .iter()
                        .map(|run| (*run).to_owned())
                        .collect::<Vec<_>>()
                ),
                "{}: the group fails with the message, its unlaunched members are cancelled, and advancing carries on",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_a_group_member_pinned_to_an_old_document_when_claiming_then_it_fails_and_another_location_is_claimed()
 {
    let test_cases = [
        ClaimOldDocumentTestCase {
            description: "SQLite store",
            backend: Backend::Sqlite,
            stored_source: "project",
            expected_claimed: "returns orders",
            expected_member: "staging_job failed: this run's snapshot was published by an earlier SQLBuild release and can't be read; rerun it after its location publishes again",
        },
        ClaimOldDocumentTestCase {
            description: "PostgreSQL store",
            backend: Backend::Postgres,
            stored_source: "default",
            expected_claimed: "returns orders",
            expected_member: "staging_job failed: this run's snapshot was published by an earlier SQLBuild release and can't be read; rerun it after its location publishes again",
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
            publish_snapshot(store, &group_snapshot(location))
                .await
                .expect("publish");
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
            advance_groups(store, &lease, (&limits, (location, "development")))
                .await
                .expect("start the group");
            store_old_documents(store, test_case.stored_source).await;
            let other = create_location(store, "returns", "test-admin")
                .await
                .expect("another location");
            publish_snapshot(store, &upgrade_snapshot(other, json!({})))
                .await
                .expect("the other location publishes");
            request_job_run(store, (&operator(), None), &manual_run(other, "orders"))
                .await
                .expect("a run in the other location");

            let claimed = claim_next_run(store, &lease, &limits, None).await;
            let claimed = claimed.map(|claim| {
                claim.map(|claim| {
                    let place = [("orders", false), ("returns", true)]
                        .into_iter()
                        .find(|(_, theirs)| *theirs == (claim.request.location_id == other))
                        .map_or("", |(place, _)| place);
                    format!("{place} {}", claim.request.job_key.unwrap_or_default())
                })
            });
            let member = run_states(store)
                .await
                .into_iter()
                .find(|run| run.starts_with("staging_job "));

            assert_eq!(
                (
                    claimed.map_or_else(|error| format!("{error:?}"), Option::unwrap_or_default),
                    member
                ),
                (
                    test_case.expected_claimed.to_owned(),
                    Some(test_case.expected_member.to_owned())
                ),
                "{}: the member whose snapshot can't be read fails, and the claim moves on",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_a_snapshot_with_an_old_jobs_document_when_checking_an_ad_hoc_plan_then_that_request_fails_with_the_message()
 {
    let test_cases = [
        AdhocOldDocumentTestCase {
            description: "SQLite store",
            backend: Backend::Sqlite,
            expected_check: UNREADABLE_PUBLICATION,
        },
        AdhocOldDocumentTestCase {
            description: "PostgreSQL store",
            backend: Backend::Postgres,
            expected_check: UNREADABLE_PUBLICATION,
        },
    ];
    for test_case in test_cases {
        run(async {
            let fixture = open_test_store(test_case.backend)
                .await
                .expect("required backend");
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let context = AuditContext {
                actor: "local-control-store-owner".into(),
                granting_role: "control-store-owner".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            let (location, snapshot_id, people) = adhoc_fixture(store, &context).await;
            store_unreadable_jobs(store, snapshot_id).await;

            let checked = check_adhoc_run(
                store,
                (&context, people.developer),
                &ExactKeyPlanner,
                &AdhocCheckRequest {
                    location_id: location,
                    target: "development".into(),
                    command: vec!["build".into(), "model:orders".into()],
                    priority: 0,
                },
            )
            .await;

            assert_eq!(
                refusal(checked),
                test_case.expected_check,
                "{}: the plan request fails with the message",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_a_running_member_when_its_group_snapshot_becomes_unreadable_then_the_group_fails_only_once_the_member_stops()
 {
    let test_cases = [
        RunningMemberOldDocumentTestCase {
            description: "SQLite store",
            backend: Backend::Sqlite,
            stored_source: "project",
            expected_while_running: &[
                "marts_job cancelled: cancelled before launch",
                "nightly_job_group running: ",
                "staging_job running: ",
            ],
            expected_cancel_requested: &["marts_job", "staging_job"],
            expected_after: &[
                "marts_job cancelled: cancelled before launch",
                "nightly_job_group failed: this run's snapshot was published by an earlier SQLBuild release and can't be read; rerun it after its location publishes again",
                "staging_job cancelled: cancelled",
            ],
        },
        RunningMemberOldDocumentTestCase {
            description: "PostgreSQL store",
            backend: Backend::Postgres,
            stored_source: "default",
            expected_while_running: &[
                "marts_job cancelled: cancelled before launch",
                "nightly_job_group running: ",
                "staging_job running: ",
            ],
            expected_cancel_requested: &["marts_job", "staging_job"],
            expected_after: &[
                "marts_job cancelled: cancelled before launch",
                "nightly_job_group failed: this run's snapshot was published by an earlier SQLBuild release and can't be read; rerun it after its location publishes again",
                "staging_job cancelled: cancelled",
            ],
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
            publish_snapshot(store, &group_snapshot(location))
                .await
                .expect("publish");
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
            let claim = claim_next_run(store, &lease, &limits, Some(scope))
                .await
                .expect("claim")
                .expect("the first member");
            report_changes(store, &claim, vec![RunChange::Started]).await;
            store_old_documents(store, test_case.stored_source).await;

            advance_groups(store, &lease, (&limits, scope))
                .await
                .expect("a pass over the unreadable group");
            let while_running = run_states(store).await;
            let cancel_requested: Vec<String> = rows(
                store,
                "SELECT job_key FROM sqb_runs WHERE cancel_requested_at IS NOT NULL ORDER BY job_key",
                vec![],
            )
            .await
            .iter()
            .map(|row| row["job_key"].as_str().unwrap_or_default().to_owned())
            .collect();
            report_changes(
                store,
                &claim,
                vec![RunChange::Finished {
                    outcome: RunOutcome::Cancelled,
                    reason: "cancelled".into(),
                    exit_code: Some(143),
                }],
            )
            .await;
            advance_groups(store, &lease, (&limits, scope))
                .await
                .expect("the next pass");

            assert_eq!(
                (while_running, cancel_requested, run_states(store).await),
                (
                    test_case
                        .expected_while_running
                        .iter()
                        .map(|run| (*run).to_owned())
                        .collect::<Vec<_>>(),
                    test_case
                        .expected_cancel_requested
                        .iter()
                        .map(|run| (*run).to_owned())
                        .collect::<Vec<_>>(),
                    test_case
                        .expected_after
                        .iter()
                        .map(|run| (*run).to_owned())
                        .collect::<Vec<_>>()
                ),
                "{}: the running member is asked to stop and keeps the group open; once it stops, the group fails",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}
