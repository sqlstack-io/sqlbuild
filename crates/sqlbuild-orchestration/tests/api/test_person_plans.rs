//! A person's plan is frozen like an ad-hoc build, queued as location work and read only by them.

use serde_json::json;
use uuid::Uuid;

use crate::api::tests::plans_helpers::{ask, person, plan_request, register_instance, run_rows};
use crate::api::tests::test_types::PlanTestCase;
use crate::authorization::main::bootstrap_user::bootstrap_user;
use crate::credentials::models::{AuditContext, BuiltinRole};
use crate::credentials::tests::location_helpers::create_location;
use crate::platform::main::claim_location_work::claim_location_work;
use crate::platform::main::complete_location_work::complete_location_work;
use crate::platform::main::read_person_plan::read_person_plan;
use crate::platform::main::request_location_work::request_location_work;
use crate::platform::models::{
    LocationWork, LocationWorkOutcome, LocationWorkRequest, LocationWorkResult, PlanRequest,
};
use crate::runs::tests::helpers::snapshot;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
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
            expected_selection: &["customers", "orders"],
            expected_warning: "connect failed: db_password=[REDACTED]",
        },
        PlanTestCase {
            description: "PostgreSQL plans",
            backend: Backend::Postgres,
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
            let first = plan_request(&[
                "build",
                "customers",
                "orders",
                "--full-refresh",
                "--warehouse",
                "WH_PLAN",
            ]);

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
                    ask(store, (&context, admin), location, &plan_request(&["build", "products"])).await,
                    Err(ControlError::Invalid(message)) if message.contains("already waiting")
                ),
                "{description}: four plans per target",
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
                ..
            } = &claimed.request.work
            else {
                panic!(
                    "{description}: plan work",
                    description = test_case.description
                )
            };
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
            assert_eq!(
                settings["warehouse"],
                json!("WH_PLAN"),
                "{description}",
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
                outcome: LocationWorkOutcome::Plan { document },
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
                "models": [{"name": "orders", "action": "create_table", "reason": "first_run"}],
                "warnings": [{"severity": "warning", "message": "connect failed: db_password=orders-pass"}],
                "python_nodes": [{"name": "orders_export", "identity_diff": {"source_diff": ["-a", "+b"]}}],
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
            let Some(LocationWorkOutcome::Plan { document }) =
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
                document["python_nodes"][0].get("identity_diff").is_none(),
                "{description}: viewers lack view.sql",
                description = test_case.description
            );
            assert!(
                read(developer).await.expect("read").is_none(),
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
