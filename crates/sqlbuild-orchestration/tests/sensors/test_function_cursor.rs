//! Only a polling sensor's function sees `ctx.cursor`; an event sensor's position belongs to the server.

use uuid::Uuid;

use crate::credentials::tests::location_helpers::create_location;
use crate::declarations::main::list_triggers::list_triggers;
use crate::scheduler::tests::contracts::helpers::acquire_lease;
use crate::sensors::main::begin_sensor_evaluation::begin_sensor_evaluation;
use crate::sensors::models::{SensorCursor, SensorEvaluationRequest};
use crate::sensors::tests::helpers::{append_events, snapshot};
use crate::sensors::tests::test_types::FunctionCursorTestCase;
use crate::snapshots::main::publication::publish_snapshot::publish_snapshot;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_a_stored_scan_position_when_a_tick_calls_the_function_then_only_a_polling_sensor_sees_a_cursor()
 {
    let test_cases = [
        FunctionCursorTestCase {
            description: "an event tick, such as a source sensor part-way through its pages",
            with_event: true,
            expected_cursor: None,
        },
        FunctionCursorTestCase {
            description: "a polling tick",
            with_event: false,
            expected_cursor: Some("page-2"),
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(Backend::Sqlite).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let location = create_location(store, "orders", "test-admin")
                .await
                .expect("location");
            publish_snapshot(store, &snapshot(location))
                .await
                .expect("publish");
            let trigger = list_triggers(store, location, "development")
                .await
                .expect("triggers")
                .remove(0);
            let lease = acquire_lease(store, (Uuid::now_v7(), None))
                .await
                .expect("elect")
                .expect("leader");
            append_events(store, 0..1, chrono::Utc::now()).await;
            let begun = begin_sensor_evaluation(store, &lease, (trigger.trigger_id, None))
                .await
                .expect("request")
                .expect("due");
            let request = SensorEvaluationRequest {
                cursor: SensorCursor {
                    custom: Some("page-2".into()),
                    ..begun.cursor.clone()
                },
                event: begun.event.clone().filter(|_| test_case.with_event),
                ..begun
            };
            assert_eq!(
                request.function_cursor(),
                test_case.expected_cursor,
                "{}",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}
