use std::sync::{Arc, Mutex};

use axum::Router;
use axum::routing::post;
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

use crate::authorization::main::people::bootstrap_user::bootstrap_user;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::credentials::tests::location_helpers::create_location;
use crate::declarations::main::list_triggers::list_triggers;
use crate::platform::main::person_reads::read_person_problems::read_person_problems;
use crate::platform::models::{PlatformEvent, SkippedEvent};
use crate::platform::tests::helpers::{drain_webhook, receive_webhook};
use crate::platform::tests::test_types::{UnreadableEventTestCase, WebhookReceiver, WebhookState};
use crate::scheduler::tests::contracts::helpers::acquire_lease;
use crate::sensors::tests::helpers::{append_events, drain_sensor, snapshot};
use crate::snapshots::main::publication::publish_snapshot::publish_snapshot;
use crate::store::main::append_control_event::append_control_event;
use crate::store::main::migrate_store::migrate_store;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlEvent, TransactionMode};
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;
use crate::webhooks::main::connect_webhook::connect_webhook;
use crate::webhooks::models::WebhookSource;

#[test]
fn given_an_unreadable_run_failure_before_good_events_when_webhooks_and_sensors_read_then_both_carry_on_and_list_it_once()
 {
    let test_cases = [
        UnreadableEventTestCase {
            description: "SQLite, webhook delivery reads first",
            backend: Backend::Sqlite,
            reader_rotation: 0,
            expected_sensor_runs: 3,
            expected_deliveries: 7,
            expected_reason: "missing field `trigger`",
        },
        UnreadableEventTestCase {
            description: "SQLite, the sensor reads first",
            backend: Backend::Sqlite,
            reader_rotation: 1,
            expected_sensor_runs: 3,
            expected_deliveries: 7,
            expected_reason: "missing field `trigger`",
        },
        UnreadableEventTestCase {
            description: "PostgreSQL, webhook delivery reads first",
            backend: Backend::Postgres,
            reader_rotation: 0,
            expected_sensor_runs: 3,
            expected_deliveries: 7,
            expected_reason: "missing field `trigger`",
        },
        UnreadableEventTestCase {
            description: "PostgreSQL, the sensor reads first",
            backend: Backend::Postgres,
            reader_rotation: 1,
            expected_sensor_runs: 3,
            expected_deliveries: 7,
            expected_reason: "missing field `trigger`",
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
                .expect("administrator");
            let location = create_location(store, "orders", "test-admin")
                .await
                .expect("location");
            let mut input = snapshot(location);
            input.documents.insert(
                "triggers".into(),
                json!({"health_orders": {
                    "description": "Refresh reporting on platform problems", "function": "start_orders",
                    "code_hash": "v1", "timeout_seconds": 60, "minimum_interval_seconds": 1,
                    "when": {"kind": "platform_event",
                        "kinds": ["run_failed", "tick_overdue", "location_broken", "launch_failed"]}}}),
            );
            publish_snapshot(store, &input)
                .await
                .expect("publish sensor");
            let trigger = list_triggers(store, location, "development")
                .await
                .expect("triggers")
                .remove(0);
            let lease = acquire_lease(store, (Uuid::now_v7(), None))
                .await
                .expect("elect")
                .expect("leader");

            // A run failure as 0.261.0 stored it, without the `trigger` this release requires, in front of good events.
            let unreadable = Uuid::now_v7();
            let transaction = begin(store, TransactionMode::Write).await.expect("write");
            let (transaction, _) = append_control_event(
                transaction,
                ControlEvent {
                    event_id: unreadable,
                    invocation_id: "platform/test-health/stored".into(),
                    event_type: "platform_run_failed",
                    producer: "platform",
                    occurred_at: Utc::now(),
                    run_id: None,
                    payload: json!({"platform": {"kind": "run_failed", "run_id": Uuid::now_v7(),
                        "run_kind": "job", "job_key": "orders", "location_id": location,
                        "target": "development", "outcome": "failed", "failed_nodes": [],
                        "failed_node_count": 0}}),
                },
            )
            .await
            .expect("an event an earlier release stored");
            commit(transaction).await.expect("stored");
            append_events(store, 0..6, Utc::now()).await;

            let state = Arc::new(Mutex::new(WebhookState {
                status: 204,
                requests: Vec::new(),
            }));
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .expect("webhook listener");
            let endpoint = format!("http://{}/events", listener.local_addr().expect("address"));
            let app = Router::new()
                .route("/events", post(receive_webhook))
                .with_state(WebhookReceiver {
                    state: state.clone(),
                    delivered: tokio::sync::watch::channel(0).0,
                });
            let (http_stop, http_shutdown) = tokio::sync::oneshot::channel();
            let server = tokio::spawn(async move {
                axum::serve(listener, app)
                    .with_graceful_shutdown(async {
                        drop(http_shutdown.await);
                    })
                    .await
            });
            let sink = connect_webhook(&endpoint, WebhookSource::Platform).expect("webhook");
            let problems = || async {
                read_person_problems(store, admin, DefaultRole::None)
                    .await
                    .expect("problems")
                    .skipped_events
            };
            // Each reader reads every stored event; the sensor's step returns the runs it started.
            let mut steps: [std::pin::Pin<Box<dyn std::future::Future<Output = usize> + '_>>; 2] = [
                Box::pin(async {
                    drain_webhook(store, &sink).await;
                    0
                }),
                Box::pin(drain_sensor(store, &lease, trigger.trigger_id)),
            ];
            steps.rotate_left(test_case.reader_rotation);
            let mut runs = 0;
            for step in steps {
                runs += step.await;
                let listed: Vec<SkippedEvent> = problems().await;
                assert_eq!(
                    listed.len(),
                    1,
                    "{}: the first reader lists the skip",
                    test_case.description
                );
                let skipped = &listed[0];
                assert_eq!(skipped.event_id, unreadable, "{}", test_case.description);
                assert_eq!(
                    skipped.event_type, "platform_run_failed",
                    "{}",
                    test_case.description
                );
                assert_eq!(
                    skipped.location_id,
                    Some(location),
                    "{}",
                    test_case.description
                );
                assert_eq!(
                    skipped.location_name.as_deref(),
                    Some("orders"),
                    "{}",
                    test_case.description
                );
                assert_eq!(
                    skipped.target.as_deref(),
                    Some("development"),
                    "{}",
                    test_case.description
                );
                assert!(
                    skipped.reason.contains(test_case.expected_reason),
                    "{}: {}",
                    test_case.description,
                    skipped.reason
                );
            }
            assert_eq!(
                runs, test_case.expected_sensor_runs,
                "{}: the sensor ticks for each readable event after the unreadable one",
                test_case.description
            );
            let delivered = state.lock().expect("receiver").requests.clone();
            assert_eq!(
                delivered.len(),
                test_case.expected_deliveries,
                "{}",
                test_case.description
            );
            assert!(
                delivered.iter().all(|event| event.event_id != unreadable),
                "{}: the unreadable event is never sent",
                test_case.description
            );
            assert_eq!(
                delivered
                    .iter()
                    .filter(|event| matches!(
                        &event.payload.platform,
                        PlatformEvent::UnreadableEventSkipped { event_id, .. } if *event_id == unreadable
                    ))
                    .count(),
                1,
                "{}: the webhook also receives the skip", test_case.description
            );
            let (transaction, rows) = query(
                begin(store, TransactionMode::Read).await.expect("read"),
                "SELECT COUNT(*) AS skips FROM recall_events WHERE event_type='platform_unreadable_event_skipped'",
                vec![],
            )
            .await
            .expect("skips");
            commit(transaction).await.expect("read");
            assert_eq!(
                serde_json::from_value::<i64>(rows[0]["skips"].clone()).expect("count"),
                1,
                "{}: the second reader skips it without recording it again",
                test_case.description
            );
            http_stop.send(()).expect("stop receiver");
            server.await.expect("server task").expect("server stopped");
            close_test_store(fixture).await;
        });
    }
}
