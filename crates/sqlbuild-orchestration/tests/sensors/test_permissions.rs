//! A sensor change needs its permission on every node of the sensor's target: toggle for pause, reset_cursor for reset and skip.

use serde_json::json;

use crate::credentials::tests::role_helpers::{
    SUPPORT_ROLE, credentials, orders_grant, role_fixture,
};
use crate::declarations::main::list_triggers::list_triggers;
use crate::declarations::models::TriggerChange;
use crate::sensors::tests::helpers::{member_sensor_change, snapshot};
use crate::sensors::tests::test_types::SensorPermissionTestCase;
use crate::snapshots::main::publication::publish_snapshot::publish_snapshot;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

const ALL_NODES: &[&str] = &["customers", "inventory", "orders", "products"];

#[test]
fn given_a_member_with_some_permissions_when_changing_a_sensor_then_each_change_needs_its_permission_on_the_whole_target()
 {
    let test_cases = [
        SensorPermissionTestCase {
            description: "SQLite: both permissions on part of the target allow nothing",
            backend: Backend::Sqlite,
            permissions: &["view", "trigger.toggle", "sensor.reset_cursor"],
            nodes: &["orders"],
            expected: [
                Err("missing trigger.toggle permission for this target"),
                Err("missing sensor.reset_cursor permission for this target"),
                Err("missing sensor.reset_cursor permission for this target"),
            ],
        },
        SensorPermissionTestCase {
            description: "PostgreSQL: toggle on the whole target pauses but neither resets nor skips",
            backend: Backend::Postgres,
            permissions: &["view", "trigger.toggle"],
            nodes: ALL_NODES,
            expected: [
                Ok(true),
                Err("missing sensor.reset_cursor permission for this target"),
                Err("missing sensor.reset_cursor permission for this target"),
            ],
        },
        SensorPermissionTestCase {
            description: "SQLite: reset_cursor on the whole target resets and reaches the skip's own checks",
            backend: Backend::Sqlite,
            permissions: &["view", "sensor.reset_cursor"],
            nodes: ALL_NODES,
            expected: [
                Err("missing trigger.toggle permission for this target"),
                Ok(true),
                Err("invalid control-plane data: the sensor hasn't been called with an event yet"),
            ],
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
                    "grants": [orders_grant(json!(test_case.permissions), test_case.nodes)]}}),
            );
            publish_snapshot(store, &input).await.expect("publish");
            let trigger = list_triggers(store, roles.location_id, "development")
                .await
                .expect("triggers")
                .remove(0);
            let mut outcomes = Vec::new();
            for change in [
                TriggerChange::Pause,
                TriggerChange::ResetCursor,
                TriggerChange::SkipEvent {
                    event: "platform:stuck".into(),
                },
            ] {
                outcomes.push(
                    member_sensor_change(
                        store,
                        (&roles.context, credentials(&roles.member)),
                        trigger.trigger_id,
                        change,
                    )
                    .await,
                );
            }
            assert_eq!(
                outcomes,
                test_case
                    .expected
                    .map(|expected| expected.map_err(str::to_owned)),
                "{}",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}
