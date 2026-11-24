//! A person's plan is frozen like an ad-hoc build, queued as location work and read only by them.

use std::collections::BTreeMap;

use serde_json::json;
use sqlbuild_core::plan_display::main::detach_model_diffs::detach_model_diffs;
use uuid::Uuid;

use crate::api::tests::plans_helpers::{ask, person, plan_request, register_instance, run_rows};
use crate::api::tests::test_types::{
    PlanDiffTestCase, PlanLoadFailureTestCase, PlanTestCase, PlanTimingsReadTestCase,
};
use crate::api::tests::work_helpers::execute;
use crate::authorization::main::people::bootstrap_user::bootstrap_user;
use crate::credentials::models::{AuditContext, BuiltinRole};
use crate::credentials::tests::location_helpers::create_location;
use crate::platform::_helpers::maintenance::retention::retain;
use crate::platform::main::health::report_location_health::report_location_health;
use crate::platform::main::person_reads::read_person_plan::read_person_plan;
use crate::platform::main::person_reads::read_person_plan_diff::read_person_plan_diff;
use crate::platform::main::person_reads::read_person_plan_rows::read_person_plan_rows;
use crate::platform::main::person_reads::read_person_plan_summary::read_person_plan_summary;
use crate::platform::main::work::claim_location_work::claim_location_work;
use crate::platform::main::work::complete_location_work::complete_location_work;
use crate::platform::main::work::request_location_work::request_location_work;
use crate::platform::models::{
    LocationHealth, LocationHealthReport, LocationWork, LocationWorkOutcome, LocationWorkRequest,
    LocationWorkResult, PlanModelDiff, PlanRequest, PlanRowSection, PlanRowsQuery, PlanTimingStep,
    PlanTimings,
};
use crate::runs::tests::helpers::snapshot;
use crate::snapshots::main::publication::publish_snapshot::publish_snapshot;
use crate::store::errors::ControlError;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_person_plans_when_requested_claimed_and_read_then_they_stay_location_work_for_their_requester()
 {
    let test_cases = [
        PlanTestCase {
            description: "SQLite plans",
            backend: Backend::Sqlite,
            fresh: true,
            reused_from: None,
            expected_selection: &["customers", "orders"],
            expected_warning: "connect failed: db_password=[REDACTED]",
        },
        PlanTestCase {
            description: "PostgreSQL plans",
            backend: Backend::Postgres,
            fresh: false,
            reused_from: Some("2026-01-02T03:04:05Z"),
            expected_selection: &["customers", "orders"],
            expected_warning: "connect failed: db_password=[REDACTED]",
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let context = AuditContext {
                actor: "control-store-owner".into(),
                granting_role: "control-store-owner".into(),
                request_id: Uuid::now_v7(),
                ip_address: Some(std::net::Ipv4Addr::LOCALHOST.into()),
            };
            let admin = bootstrap_user(store, &context, "orders-admin")
                .await
                .expect("bootstrap");
            let location = create_location(store, "orders", "test-owner")
                .await
                .expect("location");
            publish_snapshot(store, &snapshot(location))
                .await
                .expect("publish");
            let owner = (&context, admin);
            let viewer = person(
                store,
                owner,
                ("orders-viewer", location),
                &[BuiltinRole::Viewer],
            )
            .await;
            let developer = person(
                store,
                owner,
                ("orders-developer", location),
                &[BuiltinRole::Developer],
            )
            .await;
            let analyst = person(
                store,
                owner,
                ("orders-analyst", location),
                &[BuiltinRole::Viewer],
            )
            .await;
            let auditor = person(
                store,
                owner,
                ("orders-auditor", location),
                &[BuiltinRole::Viewer],
            )
            .await;
            let outsider = person(store, owner, ("orders-outsider", location), &[]).await;
            let first = PlanRequest {
                fresh: test_case.fresh,
                ..plan_request(&[
                    "build",
                    "customers",
                    "orders",
                    "--full-refresh",
                    "--warehouse",
                    "WH_PLAN",
                ])
            };

            let silent = ask(store, (&context, viewer), location, &first).await;
            assert!(
                matches!(&silent, Err(ControlError::Invalid(message)) if message.contains("sqb location serve")),
                "{description}: no live location, got {silent:?}",
                description = test_case.description
            );
            let poll = register_instance(store, location).await;
            assert!(
                matches!(
                    ask(store, (&context, outsider), location, &first).await,
                    Err(ControlError::Denied { .. })
                ),
                "{description}: view is required",
                description = test_case.description
            );
            assert!(
                matches!(
                    ask(store, (&context, viewer), location, &plan_request(&["seed", "customers"])).await,
                    Err(ControlError::Invalid(message)) if message.contains("cannot be planned")
                ),
                "{description}: only builds are planned",
                description = test_case.description
            );
            assert!(
                matches!(
                    ask(store, (&context, viewer), location, &plan_request(&["load", "--select", "customers"])).await,
                    Err(ControlError::Invalid(message)) if message.contains("cannot be planned")
                ),
                "{description}: a load is not a build preview",
                description = test_case.description
            );
            assert!(
                matches!(
                    ask(
                        store,
                        (&context, viewer),
                        location,
                        &plan_request(&["build", "orders", "--vars", r#"{"day": "7"}"#])
                    )
                    .await,
                    Err(ControlError::Denied { .. })
                ),
                "{description}: variables change the compile, so planning with them needs run.vars",
                description = test_case.description
            );
            assert!(
                ask(store, (&context, viewer), location, &first)
                    .await
                    .expect("viewer plans"),
                "{description}: a viewer may plan",
                description = test_case.description
            );
            assert!(
                !ask(store, (&context, viewer), location, &first)
                    .await
                    .expect("idempotent retry"),
                "{description}: a retry queues nothing",
                description = test_case.description
            );
            let altered = PlanRequest {
                command: vec!["build".into(), "inventory".into()],
                ..first.clone()
            };
            assert!(
                ask(store, (&context, viewer), location, &altered)
                    .await
                    .is_err(),
                "{description}: a request ID cannot carry another command",
                description = test_case.description
            );
            let refreshed = PlanRequest {
                fresh: !first.fresh,
                ..first.clone()
            };
            assert!(
                ask(store, (&context, viewer), location, &refreshed)
                    .await
                    .is_err(),
                "{description}: a request ID cannot change whether it plans again",
                description = test_case.description
            );
            assert!(
                ask(store, (&context, developer), location, &first)
                    .await
                    .is_err(),
                "{description}: a request ID cannot carry another actor",
                description = test_case.description
            );
            assert!(
                matches!(
                    ask(store, (&context, viewer), location, &plan_request(&["build", "inventory"])).await,
                    Err(ControlError::Invalid(message)) if message.contains("still running")
                ),
                "{description}: one unfinished plan per person",
                description = test_case.description
            );
            for queued in [developer, analyst, auditor] {
                assert!(
                    ask(
                        store,
                        (&context, queued),
                        location,
                        &plan_request(&["build", "products"])
                    )
                    .await
                    .expect("queue has room"),
                    "{description}: queue has room",
                    description = test_case.description
                );
            }
            assert!(
                matches!(
                    ask(store, (&context, admin), location, &plan_request(&["build", "products", "--vars", r#"{"day": "7"}"#])).await,
                    Err(ControlError::Invalid(message)) if message.contains("already waiting")
                ),
                "{description}: an admin may plan with variables, up to four plans per target",
                description = test_case.description
            );
            let direct = LocationWorkRequest {
                request_id: Uuid::now_v7(),
                location_id: location,
                target: "development".into(),
                reason: "plan".into(),
                work: LocationWork::Plan {
                    command: vec!["build".into(), "orders".into()],
                    selection: ["orders".to_owned()].into(),
                    settings: Default::default(),
                    fresh: false,
                    jobs: Default::default(),
                    given: Default::default(),
                },
            };
            assert!(
                request_location_work(store, (&context, None), &direct)
                    .await
                    .is_err(),
                "{description}: plans only come from a person's frozen command",
                description = test_case.description
            );
            let connection = LocationWorkRequest {
                request_id: Uuid::now_v7(),
                location_id: location,
                target: "development".into(),
                reason: "Check orders connectivity".into(),
                work: LocationWork::CheckConnection {},
            };
            request_location_work(store, (&context, None), &connection)
                .await
                .expect("connection test");
            let claimed = claim_location_work(store, location, &poll)
                .await
                .expect("claim")
                .expect("work");
            assert_eq!(
                claimed.request.request_id,
                connection.request_id,
                "{description}: other location work goes before plans",
                description = test_case.description
            );
            complete_location_work(
                store,
                (location, connection.request_id),
                &LocationWorkResult {
                    claim_id: claimed.claim_id,
                    session_id: poll.session_id,
                    outcome: LocationWorkOutcome::Connection { healthy: true },
                },
            )
            .await
            .expect("complete connection");
            let claimed = claim_location_work(store, location, &poll)
                .await
                .expect("claim")
                .expect("plan");
            assert_eq!(
                claimed.request.request_id,
                first.request_id,
                "{description}",
                description = test_case.description
            );
            let LocationWork::Plan {
                selection,
                settings,
                fresh,
                ..
            } = &claimed.request.work
            else {
                panic!(
                    "{description}: plan work",
                    description = test_case.description
                )
            };
            assert_eq!(
                *fresh,
                test_case.fresh,
                "{description}: the location sees whether to plan again",
                description = test_case.description
            );
            assert_eq!(
                selection.iter().map(String::as_str).collect::<Vec<_>>(),
                test_case.expected_selection,
                "{description}",
                description = test_case.description
            );
            assert_eq!(
                settings["full_refresh"],
                json!(true),
                "{description}",
                description = test_case.description
            );
            assert!(
                !settings.contains_key("warehouse"),
                "{description}: a plan never takes a named warehouse",
                description = test_case.description
            );
            let lease = (claimed.lease_expires_at - chrono::Utc::now()).num_seconds();
            assert!(
                (240..=300).contains(&lease),
                "{description}: plan lease {lease}",
                description = test_case.description
            );
            assert_eq!(
                run_rows(store).await,
                0,
                "{description}: a plan is never a run",
                description = test_case.description
            );

            let complete = |document: serde_json::Value| LocationWorkResult {
                claim_id: claimed.claim_id,
                session_id: poll.session_id,
                outcome: LocationWorkOutcome::Plan {
                    document,
                    diffs: BTreeMap::new(),
                },
            };
            for invalid in [
                json!(["models"]),
                json!({"selected_count": 1}),
                json!({"models": "x".repeat(9 * 1024 * 1024)}),
            ] {
                assert!(
                    complete_location_work(store, (location, first.request_id), &complete(invalid))
                        .await
                        .is_err(),
                    "{description}: shape and size are checked",
                    description = test_case.description
                );
            }
            let document = json!({
                "models": [{"name": "orders", "action": "create_table", "reason": "first_run", "query_diff": ["-SELECT 1", "+SELECT 2"], "config_diff": ["-a", "+b"]},
                    {"name": "customers", "action": "create_view", "reason": "no_change", "config_diff": ["-a", "+b"],
                        "config_change": "full_refresh true → false"}],
                "warnings": [{"severity": "warning", "message": "connect failed: db_password=orders-pass"}],
                "python_nodes": [{"name": "orders_export", "identity_diff": {"source_diff": ["-a", "+b"]}}],
                "timings": {"total_seconds": 1.5, "steps": [{"name": "Reading warehouse state", "seconds": 1.0}, {"name": "Other", "seconds": 0.5}], "planned_from": "synced_state",
                    "reused_from": test_case.reused_from},
            });
            assert!(
                complete_location_work(
                    store,
                    (location, first.request_id),
                    &complete(document.clone())
                )
                .await
                .expect("complete plan"),
                "{description}",
                description = test_case.description
            );
            assert!(
                !complete_location_work(store, (location, first.request_id), &complete(document))
                    .await
                    .expect("same result retried after redaction"),
                "{description}",
                description = test_case.description
            );

            let read = |reader: Uuid| {
                read_person_plan(
                    store,
                    (&context, reader),
                    (location, "development", first.request_id),
                )
            };
            let record = read(viewer).await.expect("read").expect("own plan");
            let Some(LocationWorkOutcome::Plan { document, .. }) =
                record.result_json.map(|result| result.outcome)
            else {
                panic!(
                    "{description}: plan result",
                    description = test_case.description
                )
            };
            assert_eq!(
                document["warnings"][0]["message"],
                json!(test_case.expected_warning),
                "{description}",
                description = test_case.description
            );
            assert_eq!(
                document["models"][0]["reason"],
                json!("first_run"),
                "{description}",
                description = test_case.description
            );
            assert!(
                document["python_nodes"][0].get("identity_diff").is_none()
                    && document["models"][0].get("query_diff").is_none()
                    && document["models"][0].get("config_diff").is_none(),
                "{description}: viewers lack view.sql",
                description = test_case.description
            );
            let plan = (location, "development", first.request_id);
            let summary = read_person_plan_summary(store, (&context, viewer), plan)
                .await
                .expect("summary")
                .expect("own plan");
            assert_eq!(
                summary.timings,
                Some(PlanTimings {
                    total_seconds: 1.5,
                    steps: vec![
                        PlanTimingStep {
                            name: "Reading warehouse state".to_owned(),
                            seconds: 1.0,
                        },
                        PlanTimingStep {
                            name: "Other".to_owned(),
                            seconds: 0.5,
                        },
                    ],
                    planned_from: "synced_state".to_owned(),
                    reused_from: test_case
                        .reused_from
                        .map(|at| at.parse().expect("reused_from")),
                }),
                "{description}: typed plan timings, with when a reused plan was made",
                description = test_case.description
            );
            let Some(LocationWorkOutcome::Plan {
                document: summary, ..
            }) = summary.outcome
            else {
                panic!(
                    "{description}: plan summary",
                    description = test_case.description
                )
            };
            let rows = |reader: Uuid, offset: usize| {
                read_person_plan_rows(
                    store,
                    (&context, reader),
                    plan,
                    PlanRowsQuery {
                        section: PlanRowSection::RoutineModels,
                        offset: Some(offset),
                        limit: Some(1),
                        name: None,
                    },
                )
            };
            let page = rows(viewer, 0).await.expect("rows").expect("planned");
            assert_eq!(
                (
                    summary["models"].as_array().map(Vec::len),
                    summary["models"][0].get("query_diff").is_none(),
                    summary["built_model_names"].clone(),
                    summary["pages"].clone(),
                    page.total,
                    page.rows.first().map(|row| row["name"].clone()),
                    page.rows.first().map(|row| row.get("config_diff").is_none()
                        && row.get("config_change").is_none()),
                    rows(viewer, 1)
                        .await
                        .expect("rows")
                        .map(|page| page.rows.len()),
                ),
                (
                    Some(0),
                    true,
                    serde_json::Value::Null,
                    json!({"first_run": 1, "routine_models": 1, "python_nodes": 1, "warnings": 1}),
                    1,
                    Some(json!("customers")),
                    Some(true),
                    Some(0),
                ),
                "{description}: the summary counts each section and carries no rows; pages leave diffs out",
                description = test_case.description
            );
            assert!(
                read_person_plan_diff(store, (&context, viewer), (plan, "orders"))
                    .await
                    .expect("diff")
                    .is_none(),
                "{description}: no diff without view.sql",
                description = test_case.description
            );
            assert!(
                read(developer).await.expect("read").is_none()
                    && read_person_plan_summary(store, (&context, developer), plan)
                        .await
                        .expect("summary")
                        .is_none()
                    && rows(developer, 0).await.expect("rows").is_none(),
                "{description}: only the requester reads a plan",
                description = test_case.description
            );
            assert!(
                matches!(read(outsider).await, Err(ControlError::Denied { .. })),
                "{description}: view is required to read",
                description = test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_a_plan_whose_document_left_diffs_out_when_read_then_pages_count_them_and_each_diff_is_served_on_demand()
 {
    let test_cases = [
        PlanDiffTestCase {
            description: "SQLite",
            backend: Backend::Sqlite,
            expected_detached_diff: &["-select  id", "+select id"],
        },
        PlanDiffTestCase {
            description: "PostgreSQL",
            backend: Backend::Postgres,
            expected_detached_diff: &["-select  id", "+select id"],
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let context = AuditContext {
                actor: "control-store-owner".into(),
                granting_role: "control-store-owner".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            let admin = bootstrap_user(store, &context, "orders-admin")
                .await
                .expect("bootstrap");
            let location = create_location(store, "orders", "test-owner")
                .await
                .expect("location");
            publish_snapshot(store, &snapshot(location))
                .await
                .expect("publish");
            let poll = register_instance(store, location).await;
            let request = plan_request(&["build", "orders", "customers"]);
            ask(store, (&context, admin), location, &request)
                .await
                .expect("plan");
            let claimed = claim_location_work(store, location, &poll)
                .await
                .expect("claim")
                .expect("plan work");
            let models: Vec<serde_json::Value> = (0..450)
                .map(|index| {
                    json!({"name": format!("orders_{index:03}"), "action": "create_table",
                    "reason": "query_changed", "query_diff": ["-select  id", "+select id"]})
                })
                .chain([
                    json!({"name": "totals", "action": "create_table", "reason": "config_changed",
                    "config_diff": ["-  \"full_refresh\": true,", "+  \"full_refresh\": false,"]}),
                    json!({"name": "customers", "action": "create_view", "reason": "config_changed",
                    "config_diff": ["-a", "+b"]}),
                ])
                .collect();
            let (document, detached) =
                detach_model_diffs(json!({"models": models, "warnings": []}));
            let diffs: BTreeMap<String, PlanModelDiff> =
                serde_json::from_value(json!(detached)).expect("diffs");
            let result = LocationWorkResult {
                claim_id: claimed.claim_id,
                session_id: poll.session_id,
                outcome: LocationWorkOutcome::Plan { document, diffs },
            };
            assert!(
                complete_location_work(store, (location, request.request_id), &result)
                    .await
                    .expect("complete"),
                "{}: the plan and its detached diffs are stored",
                test_case.description
            );
            assert!(
                !complete_location_work(store, (location, request.request_id), &result)
                    .await
                    .expect("retry"),
                "{}: a retried completion stores nothing twice",
                test_case.description
            );
            let plan = (location, "development", request.request_id);
            let reader = (&context, admin);
            let page = |section: PlanRowSection, name: Option<&str>| {
                read_person_plan_rows(
                    store,
                    reader,
                    plan,
                    PlanRowsQuery {
                        section,
                        offset: Some(0),
                        limit: Some(50),
                        name: name.map(str::to_owned),
                    },
                )
            };

            let changed = page(PlanRowSection::QueryChanged, None)
                .await
                .expect("rows")
                .expect("planned");
            let found = page(PlanRowSection::QueryChanged, Some("ORDERS_44"))
                .await
                .expect("rows")
                .expect("planned");
            let detached_diff = read_person_plan_diff(store, reader, (plan, "orders_123"))
                .await
                .expect("diff");
            let config_diff = read_person_plan_diff(store, reader, (plan, "customers"))
                .await
                .expect("diff");
            let missing = read_person_plan_diff(store, reader, (plan, "nowhere"))
                .await
                .expect("diff");
            let stored = read_person_plan(store, reader, plan)
                .await
                .expect("plan")
                .expect("own plan");

            assert_eq!(
                (
                    (
                        changed.total,
                        changed.rows.len(),
                        changed.rows[0]["diff_stats"].clone()
                    ),
                    (found.total, found.section_total),
                    detached_diff.map(|read| read.diff.query_diff),
                    config_diff.map(|read| read.diff.config_diff),
                    missing,
                    stored.result_json.map(|result| matches!(
                        result.outcome,
                        LocationWorkOutcome::Plan { diffs, .. } if diffs.is_empty()
                    )),
                ),
                (
                    (
                        450,
                        50,
                        json!({"query": {"added": 1, "removed": 1, "whitespace_only": true}})
                    ),
                    (10, 450),
                    Some(Some(
                        test_case
                            .expected_detached_diff
                            .iter()
                            .map(|line| (*line).to_owned())
                            .collect()
                    )),
                    Some(Some(vec!["-a".to_owned(), "+b".to_owned()])),
                    None,
                    Some(true),
                ),
                "{}: pages count the left-out diffs, each diff reads from where it was stored apart, \
                 and the stored plan never carries them",
                test_case.description
            );
            let config_changes: Vec<(serde_json::Value, Option<serde_json::Value>)> =
                page(PlanRowSection::ConfigChanged, None)
                    .await
                    .expect("rows")
                    .expect("planned")
                    .rows
                    .iter()
                    .map(|row| (row["name"].clone(), row.get("config_change").cloned()))
                    .collect();
            assert_eq!(
                config_changes,
                [
                    (json!("totals"), Some(json!("full_refresh true → false"))),
                    (json!("customers"), None),
                ],
                "{}: a short config change reads in words without its diff",
                test_case.description
            );
            execute(
                store,
                "UPDATE sqb_location_work SET expires_at='2000-01-01T00:00:00Z'",
            )
            .await;
            retain(store, Default::default(), &[])
                .await
                .expect("retention pass");
            assert_eq!(
                (
                    read_person_plan(store, reader, plan)
                        .await
                        .expect("plan")
                        .is_none(),
                    read_person_plan_diff(store, reader, (plan, "orders_123"))
                        .await
                        .expect("diff")
                        .is_none(),
                ),
                (true, true),
                "{}: an expired plan goes with the diffs it stored apart",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_a_plan_with_timings_when_its_summary_is_read_then_timings_of_another_shape_are_an_error() {
    let test_cases = [
        PlanTimingsReadTestCase {
            description: "a reused plan's timings say when it was made",
            timings: json!({"total_seconds": 1.5, "steps": [], "planned_from": "synced_state",
                "reused_from": "2026-01-02T03:04:05Z"}),
            expected_read: true,
        },
        PlanTimingsReadTestCase {
            description: "timings of another shape fail the read rather than vanish",
            timings: json!({"total_seconds": 1.5, "steps": [], "planned_from": "synced_state",
                "reused": true}),
            expected_read: false,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(Backend::Sqlite).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let context = AuditContext {
                actor: "control-store-owner".into(),
                granting_role: "control-store-owner".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            let admin = bootstrap_user(store, &context, "orders-admin")
                .await
                .expect("bootstrap");
            let location = create_location(store, "orders", "test-owner")
                .await
                .expect("location");
            publish_snapshot(store, &snapshot(location))
                .await
                .expect("publish");
            let poll = register_instance(store, location).await;
            let request = plan_request(&["build", "orders"]);
            ask(store, (&context, admin), location, &request)
                .await
                .expect("plan");
            let claimed = claim_location_work(store, location, &poll)
                .await
                .expect("claim")
                .expect("plan work");
            let result = LocationWorkResult {
                claim_id: claimed.claim_id,
                session_id: poll.session_id,
                outcome: LocationWorkOutcome::Plan {
                    document: json!({"models": [], "timings": test_case.timings}),
                    diffs: BTreeMap::new(),
                },
            };
            complete_location_work(store, (location, request.request_id), &result)
                .await
                .expect("complete");
            let plan = (location, "development", request.request_id);
            assert_eq!(
                read_person_plan_summary(store, (&context, admin), plan)
                    .await
                    .is_ok(),
                test_case.expected_read,
                "{}",
                test_case.description
            );
        });
    }
}

#[test]
fn given_a_location_whose_load_fails_when_planning_then_waiting_and_new_plans_fail_with_its_error()
{
    let test_cases = [
        PlanLoadFailureTestCase {
            description: "SQLite, the load error reported",
            backend: Backend::Sqlite,
            load_error: Some("job loaders_job: path selectors require an explicit root"),
            expected_message: "the location can't load the current code, so it can't plan: job loaders_job: path selectors require an explicit root",
        },
        PlanLoadFailureTestCase {
            description: "PostgreSQL, from a location that reports no error",
            backend: Backend::Postgres,
            load_error: None,
            expected_message: "the location can't load the current code, so it can't plan: the location's output has the error",
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let context = AuditContext {
                actor: "control-store-owner".into(),
                granting_role: "control-store-owner".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            let admin = bootstrap_user(store, &context, "orders-admin")
                .await
                .expect("bootstrap");
            let location = create_location(store, "orders", "test-owner")
                .await
                .expect("location");
            publish_snapshot(store, &snapshot(location))
                .await
                .expect("publish");
            let viewer = person(
                store,
                (&context, admin),
                ("orders-viewer", location),
                &[BuiltinRole::Viewer],
            )
            .await;
            let poll = register_instance(store, location).await;
            let waiting = plan_request(&["build", "customers"]);
            assert_eq!(
                ask(store, (&context, viewer), location, &waiting)
                    .await
                    .ok(),
                Some(true),
                "{}: a plan waits while the location loads",
                test_case.description
            );
            let reported = |health: LocationHealth| LocationHealthReport {
                event_id: Uuid::now_v7(),
                session_id: poll.session_id,
                target: "development".into(),
                health,
            };
            report_location_health(
                store,
                location,
                &reported(LocationHealth::CompileFailed {
                    generation: 2,
                    rules: None,
                    error: test_case.load_error.map(str::to_owned),
                }),
            )
            .await
            .expect("failed load");
            let answered = read_person_plan(
                store,
                (&context, viewer),
                (location, "development", waiting.request_id),
            )
            .await
            .expect("read plan")
            .and_then(|record| record.result_json);
            let Some(LocationWorkOutcome::Plan { document, .. }) =
                answered.map(|result| result.outcome)
            else {
                panic!("{}: the waiting plan failed", test_case.description);
            };
            assert_eq!(
                document["error"]["message"], test_case.expected_message,
                "{}: the waiting plan carries the load error",
                test_case.description
            );
            let refused = ask(
                store,
                (&context, viewer),
                location,
                &plan_request(&["build", "orders"]),
            )
            .await;
            assert!(
                matches!(&refused, Err(ControlError::Invalid(message)) if message == test_case.expected_message),
                "{}: a new plan fails at once, got {refused:?}",
                test_case.description
            );
            report_location_health(
                store,
                location,
                &reported(LocationHealth::Connection {
                    generation: 1,
                    healthy: true,
                    trigger: None,
                    elapsed_millis: None,
                }),
            )
            .await
            .expect("publication");
            assert_eq!(
                ask(
                    store,
                    (&context, viewer),
                    location,
                    &plan_request(&["build", "orders"])
                )
                .await
                .ok(),
                Some(true),
                "{}: once a load publishes, plans are accepted again",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}
