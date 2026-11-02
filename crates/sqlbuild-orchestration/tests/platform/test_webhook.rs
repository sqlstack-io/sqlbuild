use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Router;
use axum::routing::post;
use uuid::Uuid;

use crate::platform::main::run_watchdog::run_watchdog;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::store::main::migrate_store::migrate_store;
use crate::webhooks::_helpers::delivery::deliver_webhook;
use crate::webhooks::main::{connect_webhook::connect_webhook, run_webhook::run_webhook};
use crate::webhooks::models::WebhookSource;

use crate::credentials::tests::location_helpers::create_location;
use crate::platform::tests::helpers::{
    publish_platform_impostor, receive_webhook, snapshot, wait_for_scans, wait_for_webhook,
};
use crate::platform::tests::test_types::{PlatformSinkTestCase, WebhookReceiver, WebhookState};
use crate::store::tests::fault_helpers::{install_fault, remove_fault};
use crate::store::tests::fault_types::TransactionFault;
use crate::store::tests::replica_helpers::second_store;
use crate::store::tests::store_helpers::{close_store, close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_a_location_outage_when_webhook_delivery_retries_then_all_health_facts_survive_restart_and_lost_acknowledgements()
 {
    let test_cases = [
        PlatformSinkTestCase {
            description: "SQLite durable platform delivery",
            backend: Backend::Sqlite,
            initial_status: 503,
            expected_events: 105,
        },
        PlatformSinkTestCase {
            description: "PostgreSQL durable platform delivery",
            backend: Backend::Postgres,
            initial_status: 307,
            expected_events: 105,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate delivery store");
            let location = create_location(store, "orders", "test-admin")
                .await
                .expect("location");
            let published = publish_snapshot(store, &snapshot(location))
                .await
                .expect("publish schedules");
            // No location process and no scheduler are running.
            let replica_id = Uuid::now_v7();
            let (stop, shutdown) = tokio::sync::watch::channel(false);
            let watchdog_store = store.clone();
            let tasks = [tokio::spawn(async move {
                run_watchdog(&watchdog_store, replica_id, shutdown).await
            })];
            let observations = wait_for_scans(store, &BTreeSet::from([replica_id]), &tasks).await;
            stop.send(true).expect("stop watchdog");
            for task in tasks {
                task.await
                    .expect("watchdog task")
                    .expect("watchdog stopped");
            }
            assert_eq!(
                observations.len(),
                test_case.expected_events,
                "{}",
                test_case.description
            );
            publish_platform_impostor(store, (location, published.snapshot_id)).await;

            let state = Arc::new(Mutex::new(WebhookState {
                status: test_case.initial_status,
                requests: Vec::new(),
            }));
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .expect("webhook listener");
            let endpoint = format!(
                "http://delivery:sample-password@{}/events?token=sample-token",
                listener.local_addr().expect("address")
            );
            let receiver = WebhookReceiver {
                state: state.clone(),
                delivered: tokio::sync::watch::channel(0).0,
            };
            let app = Router::new()
                .route("/events", post(receive_webhook))
                .with_state(receiver.clone());
            let (http_stop, http_shutdown) = tokio::sync::oneshot::channel();
            let server = tokio::spawn(async move {
                axum::serve(listener, app)
                    .with_graceful_shutdown(async {
                        drop(http_shutdown.await);
                    })
                    .await
            });
            let sink =
                connect_webhook(&endpoint, WebhookSource::Platform).expect("configure webhook");
            let diagnostic = format!("{sink:?}");
            assert!(!diagnostic.contains("sample-password"));
            assert!(!diagnostic.contains("sample-token"));
            assert!(deliver_webhook(store, &sink).await.is_err());
            assert_eq!(
                state.lock().expect("receiver").requests.len(),
                1,
                "failure stops the page"
            );
            state.lock().expect("receiver").status = 204;
            let fault = TransactionFault {
                description: "lose a successful webhook acknowledgement",
                table: "sqb_platform_sink_deliveries",
                operation: "INSERT",
                condition: "",
                sqlite_body: "SELECT RAISE(ABORT, 'test receipt failure')",
                postgres_body: "RAISE EXCEPTION 'test receipt failure'",
            };
            install_fault(store, &fault).await;
            assert!(deliver_webhook(store, &sink).await.is_err());
            remove_fault(store, &fault).await;
            let second = second_store(store).await;
            let restarted = connect_webhook(&endpoint, WebhookSource::Platform)
                .expect("restart webhook client");
            let (sink_stop, sink_shutdown) = tokio::sync::watch::channel(false);
            let sink_store = second.clone();
            let dispatcher =
                tokio::spawn(
                    async move { run_webhook(&sink_store, restarted, sink_shutdown).await },
                );
            wait_for_webhook(&receiver, &dispatcher, test_case.expected_events + 2).await;
            sink_stop.send(true).expect("stop dispatcher");
            tokio::time::timeout(Duration::from_secs(2), dispatcher)
                .await
                .expect("bounded dispatcher shutdown")
                .expect("dispatcher task")
                .expect("dispatcher stopped");
            // Shutdown may cancel the last receipt after the remote endpoint accepted it.
            assert!(
                deliver_webhook(store, &sink)
                    .await
                    .expect("reconcile in-flight delivery")
                    <= 1
            );
            assert_eq!(
                deliver_webhook(store, &sink).await.expect("durable cursor"),
                0
            );
            {
                let receiver = state.lock().expect("receiver");
                let requests = &receiver.requests;
                assert!(
                    (test_case.expected_events + 2..=test_case.expected_events + 3)
                        .contains(&requests.len())
                );
                assert_eq!(requests[0], requests[1], "HTTP retry retains full identity");
                assert_eq!(
                    requests[1], requests[2],
                    "lost receipt retries the same fact"
                );
                assert_eq!(
                    requests[2..]
                        .iter()
                        .map(|event| event.event_id)
                        .collect::<BTreeSet<_>>()
                        .len(),
                    test_case.expected_events
                );
                let mut delivered = requests[2..].to_vec();
                delivered.dedup();
                assert_eq!(delivered.len(), observations.len());
                for (event, observation) in delivered.iter().zip(&observations) {
                    assert_eq!(event.storage_id, observation.storage_id);
                    assert_eq!(event.occurred_at, observation.occurred_at);
                    assert_eq!(event.payload.platform, observation.payload.platform);
                }
            }
            http_stop.send(()).expect("stop receiver");
            tokio::time::timeout(Duration::from_secs(2), server)
                .await
                .expect("bounded shutdown")
                .expect("server task")
                .expect("server stopped");
            close_store(second).await;
            close_test_store(fixture).await;
        });
    }
}
