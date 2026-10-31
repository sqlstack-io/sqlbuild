use std::collections::BTreeMap;

use axum::Router;
use axum::response::Redirect;
use axum::routing::get;
use chrono::Utc;
use sqlbuild_orchestration::credentials::main::issue_token::issue_token;
use sqlbuild_orchestration::credentials::models::{AuditContext, CredentialScope};
use sqlbuild_orchestration::runs::main::change_run::change_run;
use sqlbuild_orchestration::runs::main::claim_next_run::claim_next_run;
use sqlbuild_orchestration::runs::main::list_runs::list_runs;
use sqlbuild_orchestration::runs::models::{
    NodeOutcome, RunAuthority, RunChange, RunChangeRequest, RunOutcome, RunStatus,
};
use sqlbuild_orchestration::scheduler::main::acquire_lease::acquire_lease;
use sqlbuild_orchestration::snapshots::main::publish_snapshot::publish_snapshot;
use sqlbuild_orchestration::store::errors::ControlError;
use sqlbuild_orchestration::store::main::migrate_store::migrate_store;
use sqlbuild_orchestration::worker::errors::WorkerError;
use sqlbuild_orchestration::worker::main::connect_worker::connect_worker;
use sqlbuild_orchestration::worker::main::start_worker::start_worker;
use uuid::Uuid;

use crate::helpers::{start_api, stop_api};
use crate::location_helpers::create_location;
use crate::output_helpers::{chunk, output_contract};
use crate::recall_helpers::recall_contract;
use crate::run_helpers::{queue, request, snapshot};
use crate::store_helpers::{close_test_store, open_test_store, run};
use crate::store_test_types::Backend;
use crate::test_types::{
    ApiContractTestCase, TestApi, WorkerDeadlineTestCase, WorkerRedirectTestCase,
};

#[test]
fn given_a_claimed_run_when_a_worker_connects_then_verification_precedes_start_and_reports_are_retryable()
 {
    let test_cases = [
        ApiContractTestCase {
            description: "SQLite worker HTTP lifecycle",
            backend: Backend::Sqlite,
            expected_status: RunStatus::Success,
        },
        ApiContractTestCase {
            description: "PostgreSQL worker HTTP lifecycle",
            backend: Backend::Postgres,
            expected_status: RunStatus::Success,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let location = create_location(store, "orders", "test-admin")
                .await
                .expect("location");
            let baseline = snapshot(location);
            let publication = publish_snapshot(store, &baseline).await.expect("publish");
            queue(
                store,
                &request((location, publication.snapshot_id), 1, 0, &["orders"]),
            )
            .await;
            let lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("acquire")
                .expect("leader");
            let claim = claim_next_run(store, &lease, &BTreeMap::new(), None)
                .await
                .expect("claim")
                .expect("work");
            let identity = (claim.request.run_id, claim.claim_id);
            let token = issue_token(
                store,
                &AuditContext {
                    actor: "test-admin".into(),
                    granting_role: "admin".into(),
                    request_id: Uuid::now_v7(),
                    ip_address: None,
                },
                CredentialScope::Run {
                    run_id: identity.0,
                    claim_id: identity.1,
                },
                Utc::now() + chrono::Duration::hours(1),
            )
            .await
            .expect("worker token");
            let api = start_api(store).await;
            let worker = connect_worker(&api.url, &token.bearer, identity).expect("worker client");
            assert!(
                !format!("{worker:?}").contains(&token.bearer),
                "{}",
                test_case.description
            );
            let wrong_claim = connect_worker(&api.url, &token.bearer, (identity.0, Uuid::now_v7()))
                .expect("different local claim");
            assert!(
                matches!(
                    wrong_claim.execution().await,
                    Err(WorkerError::Configuration(_))
                ),
                "{}",
                test_case.description
            );
            let mut drifted = baseline.clone();
            drifted
                .nodes
                .get_mut("inventory")
                .expect("unselected dependency")
                .raw_sql = Some("select 2 as id".into());
            assert!(
                matches!(
                    start_worker(&worker, &drifted).await,
                    Err(WorkerError::Verification(_))
                ),
                "{}",
                test_case.description
            );
            let runs = list_runs(store, (location, "development"), 10)
                .await
                .expect("unstarted");
            assert_eq!(
                runs[0].status,
                RunStatus::Queued,
                "{}",
                test_case.description
            );
            change_run(
                store,
                RunAuthority::Scheduler(&lease),
                &RunChangeRequest {
                    event_id: Uuid::now_v7(),
                    run_id: identity.0,
                    claim_id: Some(identity.1),
                    change: RunChange::Launched { container_id: None },
                },
            )
            .await
            .expect("record process launch");
            let session = start_worker(&worker, &baseline)
                .await
                .expect("verified start");
            output_contract(store, &worker, identity.1).await;
            let recall_batch = recall_contract(store, &worker, identity.1).await;
            assert_eq!(session.execution.claim, claim, "{}", test_case.description);
            assert_eq!(
                start_worker(&worker, &baseline)
                    .await
                    .expect("retry start")
                    .execution,
                session.execution,
                "{}",
                test_case.description
            );
            assert!(
                !worker.heartbeat().await.expect("heartbeat"),
                "{}",
                test_case.description
            );
            let started = Uuid::now_v7();
            assert!(
                worker
                    .report(
                        started,
                        RunChange::NodeStarted {
                            node_key: "orders".into()
                        }
                    )
                    .await
                    .expect("node start"),
                "{}",
                test_case.description
            );
            assert!(
                !worker
                    .report(
                        started,
                        RunChange::NodeStarted {
                            node_key: "orders".into()
                        }
                    )
                    .await
                    .expect("retry node start"),
                "{}",
                test_case.description
            );
            worker
                .report(
                    Uuid::now_v7(),
                    RunChange::NodeFinished {
                        node_key: "orders".into(),
                        outcome: NodeOutcome::Success,
                        rows_affected: Some(7),
                        error_code: None,
                        skipped_because_of: None,
                    },
                )
                .await
                .expect("node outcome");
            worker
                .report(
                    Uuid::now_v7(),
                    RunChange::Finished {
                        outcome: RunOutcome::Success,
                        reason: "warehouse work completed".into(),
                        exit_code: Some(0),
                    },
                )
                .await
                .expect("finish");
            assert!(matches!(
                worker.publish_recall(&recall_batch).await,
                Err(WorkerError::Rejected(reqwest::StatusCode::UNAUTHORIZED))
            ));
            assert!(matches!(
                worker
                    .publish_output(&[chunk(
                        5,
                        sqlbuild_recall::records::models::OutputStream::Stdout,
                        b"late"
                    )])
                    .await,
                Err(WorkerError::Rejected(reqwest::StatusCode::UNAUTHORIZED))
            ));
            let runs = list_runs(store, (location, "development"), 10)
                .await
                .expect("completed");
            assert_eq!(
                runs[0].status, test_case.expected_status,
                "{}",
                test_case.description
            );
            assert!(
                matches!(
                    worker.heartbeat().await,
                    Err(WorkerError::Rejected(reqwest::StatusCode::UNAUTHORIZED))
                ),
                "{}",
                test_case.description
            );
            queue(
                store,
                &request((location, publication.snapshot_id), 2, 0, &["orders"]),
            )
            .await;
            let cancelled_claim = claim_next_run(store, &lease, &BTreeMap::new(), None)
                .await
                .expect("claim another run")
                .expect("queued work");
            let cancelled_identity = (cancelled_claim.request.run_id, cancelled_claim.claim_id);
            let cancelled_token = issue_token(
                store,
                &AuditContext {
                    actor: "test-admin".into(),
                    granting_role: "admin".into(),
                    request_id: Uuid::now_v7(),
                    ip_address: None,
                },
                CredentialScope::Run {
                    run_id: cancelled_identity.0,
                    claim_id: cancelled_identity.1,
                },
                Utc::now() + chrono::Duration::hours(1),
            )
            .await
            .expect("another worker credential");
            change_run(
                store,
                RunAuthority::Scheduler(&lease),
                &RunChangeRequest {
                    event_id: Uuid::now_v7(),
                    run_id: cancelled_identity.0,
                    claim_id: Some(cancelled_identity.1),
                    change: RunChange::Launched { container_id: None },
                },
            )
            .await
            .expect("another launched process");
            change_run(
                store,
                RunAuthority::Operator(&AuditContext {
                    actor: "test-operator".into(),
                    granting_role: "operator".into(),
                    request_id: Uuid::now_v7(),
                    ip_address: None,
                }),
                &RunChangeRequest {
                    event_id: Uuid::now_v7(),
                    run_id: cancelled_identity.0,
                    claim_id: Some(cancelled_identity.1),
                    change: RunChange::CancelRequested {
                        actor: "test-operator".into(),
                        reason: "Stop before execution".into(),
                    },
                },
            )
            .await
            .expect("cancel before execution");
            let cancelled_worker =
                connect_worker(&api.url, &cancelled_token.bearer, cancelled_identity)
                    .expect("cancelled worker connection");
            assert!(
                matches!(
                    start_worker(&cancelled_worker, &baseline).await,
                    Err(WorkerError::Cancelled)
                ),
                "{}",
                test_case.description
            );
            let runs = list_runs(store, (location, "development"), 10)
                .await
                .expect("cancelled run has not started");
            let cancelled_run = runs
                .iter()
                .find(|run| run.run_id == cancelled_identity.0)
                .expect("cancelled projection");
            assert_eq!(
                cancelled_run.status,
                RunStatus::Launched,
                "{}",
                test_case.description
            );
            assert!(
                cancelled_run.started_at.is_none(),
                "{}",
                test_case.description
            );
            drop(session);
            stop_api(api).await;
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_a_credentialed_worker_when_the_server_redirects_then_the_redirect_is_not_followed() {
    let test_cases = [WorkerRedirectTestCase {
        description: "a run bearer must not be forwarded by an API redirect",
        expected_status: reqwest::StatusCode::TEMPORARY_REDIRECT,
    }];
    for test_case in test_cases {
        run(async {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .expect("isolated redirect server");
            let url = format!("http://{}", listener.local_addr().expect("address"));
            let application = Router::new().route(
                "/api/runs/{run_id}/execution",
                get(|| async { Redirect::temporary("/must-not-receive-a-credential") }),
            );
            let (shutdown, signal) = tokio::sync::oneshot::channel();
            let task = tokio::spawn(async move {
                axum::serve(listener, application)
                    .with_graceful_shutdown(async {
                        let _ = signal.await;
                    })
                    .await
                    .map_err(ControlError::from)
            });
            let api = TestApi {
                url,
                shutdown,
                task,
            };
            let client = connect_worker(
                &api.url,
                "synthetic-worker-bearer",
                (Uuid::now_v7(), Uuid::now_v7()),
            )
            .expect("worker client");
            let Err(WorkerError::Rejected(status)) = client.execution().await else {
                panic!("{}", test_case.description);
            };
            assert_eq!(
                status, test_case.expected_status,
                "{}",
                test_case.description
            );
            stop_api(api).await;
        });
    }
}

#[test]
fn given_a_slow_recall_server_when_the_delivery_deadline_expires_then_the_native_store_stops_waiting()
 {
    use sqlbuild_recall::records::models::RowEnvelope;
    use sqlbuild_recall::store::errors::StoreErrorKind;
    use sqlbuild_recall::writer::models::SessionStore;
    use std::time::{Duration, Instant};

    let test_cases = [
        WorkerDeadlineTestCase {
            description: "HTTP wait expires within the close budget",
            budget: Duration::from_millis(50),
            expected_kind: StoreErrorKind::Busy,
            expected_arrival: true,
        },
        WorkerDeadlineTestCase {
            description: "expired budget sends no request",
            budget: Duration::ZERO,
            expected_kind: StoreErrorKind::Busy,
            expected_arrival: false,
        },
    ];
    for test_case in test_cases {
        run(async {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind slow server");
            let url = format!(
                "http://{}",
                listener.local_addr().expect("slow server address")
            );
            let arrived = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let observed = arrived.clone();
            let application = Router::new().route(
                "/api/runs/{run_id}/recall",
                axum::routing::post(move || {
                    let arrived = arrived.clone();
                    async move {
                        arrived.store(true, std::sync::atomic::Ordering::SeqCst);
                        tokio::time::sleep(Duration::from_secs(2)).await;
                        axum::Json(sqlbuild_recall::store::models::InsertOutcome::default())
                    }
                }),
            );
            let task = tokio::spawn(async move { axum::serve(listener, application).await });
            let client = connect_worker(
                &url,
                "synthetic-worker-bearer",
                (Uuid::now_v7(), Uuid::now_v7()),
            )
            .expect("connect worker to slow server");
            let SessionStore::Hosted(opener) = client.recall_store() else {
                panic!("hosted store")
            };
            tokio::task::spawn_blocking(move || {
                let envelope = RowEnvelope {
                    invocation_id: "deadline".into(),
                    invocation_started_at: "2026-01-02T03:04:05Z".into(),
                    producer_version: "test".into(),
                };
                let store = opener
                    .open(&envelope, Instant::now() + Duration::from_secs(1))
                    .expect("open HTTP store");
                let started = Instant::now();
                let error = store
                    .insert(&[], started + test_case.budget)
                    .expect_err("deadline rejects delivery");
                assert_eq!(
                    error.kind, test_case.expected_kind,
                    "{}",
                    test_case.description
                );
                assert!(
                    started.elapsed() < Duration::from_secs(1),
                    "{}",
                    test_case.description
                );
            })
            .await
            .expect("join slow-server publication");
            assert_eq!(
                observed.load(std::sync::atomic::Ordering::SeqCst),
                test_case.expected_arrival,
                "{}",
                test_case.description
            );
            task.abort();
            let _ = task.await;
        });
    }
}
