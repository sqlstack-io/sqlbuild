//! A person reads a sensor's cursor, checks and run requests behind the listing gate; custom values pass through secret redaction.

use std::time::Duration;

use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

use crate::credentials::main::identities::create_principal::create_principal;
use crate::credentials::models::{BuiltinRole, DefaultRole, RoleAssignmentRequest};
use crate::credentials::tests::role_helpers::{
    SUPPORT_ROLE, assign, credentials, orders_grant, role_fixture,
};
use crate::declarations::main::list_triggers::list_triggers;
use crate::scheduler::tests::contracts::helpers::acquire_lease;
use crate::sensors::main::begin_sensor_evaluation::begin_sensor_evaluation;
use crate::sensors::main::complete_builtin_sensor::complete_builtin_sensor;
use crate::sensors::main::read_person_sensor::read_person_sensor;
use crate::sensors::models::PersonSensorCursor;
use crate::sensors::tests::helpers::{append_events, snapshot};
use crate::sensors::tests::test_types::SensorTestCase;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::store::errors::ControlError;
use crate::store::main::migrate_store::migrate_store;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{Parameter, TransactionMode};
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_a_checked_sensor_when_people_read_its_state_then_listing_view_gates_it_and_custom_values_are_redacted()
 {
    let test_cases = [
        SensorTestCase {
            description: "SQLite",
            backend: Backend::Sqlite,
            expected_runs: 50,
        },
        SensorTestCase {
            description: "PostgreSQL",
            backend: Backend::Postgres,
            expected_runs: 50,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let roles = role_fixture(store).await;
            let mut input = snapshot(roles.location_id);
            input.generation = 2;
            input.documents.insert(
                "roles".into(),
                json!({SUPPORT_ROLE: {"description": "Answer order questions",
                    "grants": [orders_grant(json!(["view"]), &["orders"])]}}),
            );
            publish_snapshot(store, &input)
                .await
                .expect("publish sensor");
            let trigger = list_triggers(store, roles.location_id, "development")
                .await
                .expect("triggers")
                .remove(0);
            let lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("elect")
                .expect("leader");
            append_events(store, 0..105, Utc::now()).await;
            let first = begin_sensor_evaluation(store, &lease, trigger.trigger_id)
                .await
                .expect("request")
                .expect("due");
            let first_runs = complete_builtin_sensor(store, &lease, &first)
                .await
                .expect("first page");
            tokio::time::sleep(Duration::from_secs(1)).await;
            let second = begin_sensor_evaluation(store, &lease, trigger.trigger_id)
                .await
                .expect("next page")
                .expect("due");
            let second_runs = complete_builtin_sensor(store, &lease, &second)
                .await
                .expect("second page");
            let administrator = (&roles.context, credentials(&roles.administrator));
            let read = |who, trigger_id, limit| {
                read_person_sensor(
                    store,
                    who,
                    ((roles.location_id, "development"), (trigger_id, limit)),
                    DefaultRole::None,
                )
            };

            let state = read(administrator, trigger.trigger_id, 100)
                .await
                .expect("administrator")
                .expect("published sensor");
            let checks = &state.evaluations;
            assert_eq!(
                (
                    checks
                        .iter()
                        .map(|check| (check.evaluation_id, check.phase.as_str(), check.run_count))
                        .collect::<Vec<_>>(),
                    checks[0].cursor_before.event_storage_id,
                    checks[1].cursor_after.clone(),
                    state.cursor.clone(),
                    state.run_requests.len(),
                ),
                (
                    vec![
                        (second.evaluation_id, "completed", second_runs),
                        (first.evaluation_id, "completed", test_case.expected_runs),
                    ],
                    second.cursor.event_storage_id,
                    Some(checks[0].cursor_before.clone()),
                    checks[0].cursor_after.clone(),
                    test_case.expected_runs + second_runs,
                ),
                "{}: checks newest first, each starting where the last ended, with the runs they requested",
                test_case.description
            );
            assert!(
                first_runs == test_case.expected_runs
                    && checks.iter().all(|check| check.finished_at.is_some()
                        && check.definition_fingerprint == state.definition_fingerprint),
                "{}: both checks finished on the published definition",
                test_case.description
            );
            let newest = read(administrator, trigger.trigger_id, 1)
                .await
                .expect("administrator")
                .expect("published sensor");
            assert_eq!(
                (
                    newest
                        .evaluations
                        .iter()
                        .map(|check| check.evaluation_id)
                        .collect::<Vec<_>>(),
                    newest.run_requests.len()
                ),
                (vec![second.evaluation_id], 1),
                "{}: the limit keeps the newest check and run request",
                test_case.description
            );

            let transaction = begin(store, TransactionMode::Write).await.expect("write");
            let (transaction, _) = query(
                transaction,
                "UPDATE sqb_trigger_state SET cursor_json=$1 WHERE trigger_id=$2",
                vec![
                    Parameter::Json(json!({"event_storage_id": 7,
                        "custom": "postgres://orders:orders-db-pass@warehouse.example.test/orders"})),
                    Parameter::Id(trigger.trigger_id),
                ],
            )
            .await
            .expect("custom cursor");
            commit(transaction).await.expect("commit");
            let custom = read(administrator, trigger.trigger_id, 1)
                .await
                .expect("administrator")
                .expect("published sensor")
                .cursor
                .and_then(|cursor| cursor.custom);
            assert_eq!(
                custom.as_deref(),
                Some("postgres://orders:[REDACTED]@warehouse.example.test/orders"),
                "{}: a custom cursor passes through secret redaction",
                test_case.description
            );
            let viewer = create_principal(store, &roles.context, "orders-viewer")
                .await
                .expect("viewer");
            assign(
                store,
                (&roles.context, &roles.administrator),
                RoleAssignmentRequest {
                    principal_id: viewer,
                    role: BuiltinRole::Viewer.into(),
                    location_id: None,
                    target: None,
                    revoke: false,
                },
            )
            .await;
            let viewed = read((&roles.context, viewer), trigger.trigger_id, 1)
                .await
                .expect("viewer lists the target")
                .expect("published sensor");
            assert_eq!(
                (
                    viewed.cursor,
                    viewed.evaluations[0].cursor_after.clone(),
                    viewed
                        .run_requests
                        .iter()
                        .map(|request| request.run_key.as_str())
                        .collect::<Vec<_>>(),
                ),
                (
                    Some(PersonSensorCursor {
                        event_storage_id: 7,
                        custom: None
                    }),
                    checks[0].cursor_after.clone(),
                    vec![""],
                ),
                "{}: view without view.evidence sees positions but not what the sensor computed",
                test_case.description
            );

            assert!(
                read(administrator, Uuid::now_v7(), 100)
                    .await
                    .expect("administrator")
                    .is_none(),
                "{}: an unknown sensor is not found",
                test_case.description
            );
            let member = (&roles.context, roles.member_id);
            for trigger_id in [trigger.trigger_id, Uuid::now_v7()] {
                assert!(
                    matches!(
                        read(member, trigger_id, 100).await,
                        Err(ControlError::Denied { .. })
                    ),
                    "{}: view on one node does not list the target, and refusal hides whether the sensor exists",
                    test_case.description
                );
            }
            close_test_store(fixture).await;
        });
    }
}
