//! Real SQLite broadcast and PostgreSQL LISTEN/NOTIFY feed independent HTTP observers.

use std::time::Duration;

use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

use crate::api::tests::changes_helpers::{
    connect, expect_topics, next_topic, reject_snapshot_commit, reset_proxy,
    restore_snapshot_commits,
};
use crate::api::tests::helpers::{read_json, start_api, stop_api};
use crate::api::tests::test_types::ChangesTestCase;
use crate::credentials::main::{issue_token::issue_token, revoke_token::revoke_token};
use crate::credentials::models::{AuditContext, CredentialScope};
use crate::credentials::tests::location_helpers::create_location;
use crate::declarations::main::{change_trigger::change_trigger, list_triggers::list_triggers};
use crate::declarations::models::{TriggerChange, TriggerChangeRequest};
use crate::runs::main::request_job_run::request_job_run;
use crate::runs::models::JobRunRequest;
use crate::runs::tests::helpers::snapshot;
use crate::scheduler::models::TickDecision;
use crate::scheduler::tests::contracts::helpers::{acquire_lease, evaluate_trigger};
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::store::main::migrate_store::migrate_store;
use crate::store::models::ControlStore;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_independent_http_observers_when_committed_state_changes_then_scoped_sse_refetches_real_state_and_stops_on_revocation()
 {
    let test_cases = [
        ChangesTestCase {
            description: "SQLite committed live updates",
            backend: Backend::Sqlite,
            expected_generation: 2,
        },
        ChangesTestCase {
            description: "PostgreSQL replicated live updates",
            backend: Backend::Postgres,
            expected_generation: 2,
        },
    ];
    for test_case in test_cases {
        run(async {
            let fixture = open_test_store(test_case.backend)
                .await
                .expect("isolated store");
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let location_id = create_location(store, "orders", "test-admin")
                .await
                .expect("location");
            let other = create_location(store, "inventory", "test-admin")
                .await
                .expect("other location");
            let audit = AuditContext {
                actor: "test-admin".into(),
                granting_role: "admin".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            let token = issue_token(
                store,
                &audit,
                CredentialScope::Location { location_id },
                Utc::now() + chrono::Duration::hours(1),
            )
            .await
            .expect("location token");
            let mut input = snapshot(location_id);
            input.documents.insert("jobs".into(), json!({"orders": {"description":"Build orders", "selection":["orders"], "overlap":"allow"}}));
            input.documents.insert("triggers".into(), json!({"daily_orders": {"description":"Refresh orders", "job_key":"orders", "starts_at":"2020-01-01T00:00:00Z",
                "schedule":{"kind":"calendar", "times":["2020-01-01T00:00:00Z","2030-01-01T00:00:00Z"], "refresh_at":"2029-01-01T00:00:00Z"}}}));
            publish_snapshot(store, &input)
                .await
                .expect("initial snapshot");
            let first = start_api(store).await;
            let second = start_api(store).await;
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("HTTP client");
            let scope = (location_id, "development", token.bearer.as_str());
            let mut a = connect(&client, &first, scope).await;
            let mut b = connect(&client, &second, scope).await;
            let path = format!(
                "{}/api/locations/{location_id}/targets/development",
                first.url
            );
            assert_eq!(
                client
                    .get(format!("{path}/changes"))
                    .send()
                    .await
                    .expect("anonymous request")
                    .status(),
                reqwest::StatusCode::UNAUTHORIZED
            );
            assert_eq!(
                client
                    .get(format!(
                        "{}/api/locations/{other}/targets/development/changes",
                        first.url
                    ))
                    .bearer_auth(&token.bearer)
                    .send()
                    .await
                    .expect("foreign subscription")
                    .status(),
                reqwest::StatusCode::FORBIDDEN
            );
            let mut foreign = input.clone();
            foreign.location_id = other;
            publish_snapshot(store, &foreign)
                .await
                .expect("foreign publication");
            foreign.location_id = location_id;
            foreign.target = "preview".into();
            publish_snapshot(store, &foreign)
                .await
                .expect("other target");
            input.generation = 2;
            let response = client
                .post(format!("{path}/snapshots"))
                .bearer_auth(&token.bearer)
                .json(&input)
                .send()
                .await
                .expect("HTTP publish");
            assert_eq!(response.status(), reqwest::StatusCode::OK);
            let snapshot_topics = [
                format!("snapshot:{location_id}"),
                "node_status:development".into(),
            ];
            expect_topics(&mut a, &snapshot_topics).await;
            expect_topics(&mut b, &snapshot_topics).await;
            let current = read_json(
                &client,
                (
                    &format!(
                        "{}/api/locations/{location_id}/targets/development/snapshots",
                        second.url
                    ),
                    &token.bearer,
                ),
            )
            .await;
            assert_eq!(
                current["record"]["generation"], test_case.expected_generation,
                "{}: notification observes committed state on the other replica",
                test_case.description
            );

            reject_snapshot_commit(store).await;
            input.generation = 3;
            assert!(
                publish_snapshot(store, &input).await.is_err(),
                "late deferred constraint rejects COMMIT after all snapshot work"
            );
            restore_snapshot_commits(store).await;
            let current = read_json(&client, (&format!("{path}/snapshots"), &token.bearer)).await;
            assert_eq!(current["record"]["generation"], 2);
            let request = JobRunRequest {
                run_id: Uuid::now_v7(),
                location_id,
                target: "development".into(),
                job_key: "orders".into(),
                reason: "manual verification".into(),
                priority: 0,
                window: None,
            };
            assert!(
                request_job_run(store, &audit, &request)
                    .await
                    .expect("queue job")
            );
            let run_topics = [
                "runs".into(),
                format!("run:{}", request.run_id),
                "node_status:development".into(),
            ];
            expect_topics(&mut a, &run_topics).await;
            expect_topics(&mut b, &run_topics).await;
            assert!(
                !request_job_run(store, &audit, &request)
                    .await
                    .expect("idempotent retry")
            );
            let trigger_id = list_triggers(store, location_id, "development")
                .await
                .expect("trigger")[0]
                .trigger_id;
            let lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("acquire lease")
                .expect("scheduler elected");
            let plan = evaluate_trigger(store, &lease, trigger_id)
                .await
                .expect("tick decision")
                .expect("active trigger");
            let TickDecision::Requested { run_id } = plan.actions[0].decision else {
                panic!("new scheduled run")
            };
            let tick_topics = [
                "ticks".into(),
                "runs".into(),
                format!("run:{run_id}"),
                "node_status:development".into(),
            ];
            expect_topics(&mut a, &tick_topics).await;
            expect_topics(&mut b, &tick_topics).await;
            change_trigger(
                store,
                &audit,
                &TriggerChangeRequest {
                    event_id: Uuid::now_v7(),
                    trigger_id,
                    actor: audit.actor.clone(),
                    reason: "pause verification".into(),
                    change: TriggerChange::Pause,
                },
            )
            .await
            .expect("pause");
            expect_topics(&mut a, &["ticks".into()]).await;
            expect_topics(&mut b, &["ticks".into()]).await;

            assert!(
                revoke_token(store, &audit, token.identity.token_id)
                    .await
                    .expect("revoke")
            );
            assert_eq!(
                next_topic(&mut a).await,
                None,
                "revocation closes an existing stream"
            );
            assert_eq!(
                next_topic(&mut b).await,
                None,
                "revocation reaches another replica"
            );
            let fresh = issue_token(
                store,
                &audit,
                CredentialScope::Location { location_id },
                Utc::now() + chrono::Duration::hours(1),
            )
            .await
            .expect("new token");
            let mut live =
                connect(&client, &first, (location_id, "development", &fresh.bearer)).await;
            let expiring = issue_token(
                store,
                &audit,
                CredentialScope::Location { location_id },
                Utc::now() + chrono::Duration::seconds(1),
            )
            .await
            .expect("expiring token");
            let mut expires = connect(
                &client,
                &second,
                (location_id, "development", &expiring.bearer),
            )
            .await;
            tokio::time::sleep(
                (expiring.identity.expires_at - Utc::now())
                    .to_std()
                    .expect("token is still active")
                    + Duration::from_millis(10),
            )
            .await;
            publish_snapshot(store, &input)
                .await
                .expect("committed change after expiry");
            expect_topics(&mut live, &snapshot_topics).await;
            assert_eq!(
                next_topic(&mut expires).await,
                None,
                "expired streams disclose no subsequent changes"
            );
            // Reconnect starts with all, then REST supplies the current state rather than a replay.
            let mut reconnected = connect(
                &client,
                &second,
                (location_id, "development", &fresh.bearer),
            )
            .await;
            let current = read_json(&client, (&format!("{path}/snapshots"), &fresh.bearer)).await;
            assert_eq!(current["record"]["generation"], 3);
            let long_target = "p".repeat(9_000);
            let mut long_stream =
                connect(&client, &second, (location_id, &long_target, &fresh.bearer)).await;
            input.target = long_target.clone();
            publish_snapshot(store, &input)
                .await
                .expect("target larger than PostgreSQL's NOTIFY payload limit");
            expect_topics(
                &mut long_stream,
                &[
                    format!("snapshot:{location_id}"),
                    format!("node_status:{long_target}"),
                ],
            )
            .await;
            stop_api(first).await;
            assert_eq!(
                next_topic(&mut live).await,
                None,
                "shutdown drains a live SSE body"
            );
            stop_api(second).await;
            assert_eq!(next_topic(&mut reconnected).await, None);
            assert_eq!(next_topic(&mut long_stream).await, None);
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_terminated_postgres_listeners_when_reconnecting_then_http_survives_and_observers_refetch()
{
    let test_cases = [ChangesTestCase {
        description: "PostgreSQL listener interruption",
        backend: Backend::Postgres,
        expected_generation: 2,
    }];
    for test_case in test_cases {
        run(async {
            let fixture = open_test_store(test_case.backend)
                .await
                .expect("isolated store");
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let location_id = create_location(store, "orders", "test-admin")
                .await
                .expect("location");
            let audit = AuditContext {
                actor: "test-admin".into(),
                granting_role: "admin".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            let token = issue_token(
                store,
                &audit,
                CredentialScope::Location { location_id },
                Utc::now() + chrono::Duration::hours(1),
            )
            .await
            .expect("token");
            let mut input = snapshot(location_id);
            publish_snapshot(store, &input).await.expect("snapshot");
            let proxy = reset_proxy(store).await;
            let first = start_api(&proxy.store).await;
            let second = start_api(&proxy.store).await;
            let client = reqwest::Client::new();
            let scope = (location_id, "development", token.bearer.as_str());
            let mut a = connect(&client, &first, scope).await;
            let mut b = connect(&client, &second, scope).await;
            let ControlStore::Postgres(pool) = store else {
                panic!("PostgreSQL fixture")
            };
            let terminated: Vec<bool> = sqlx_core::query_scalar::query_scalar(
                "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE pid<>pg_backend_pid() AND query LIKE 'LISTEN%' AND query LIKE '%' || md5(current_schema()) || '%'",
            ).fetch_all(pool).await.expect("interrupt only this fixture's LISTEN connections");
            assert_eq!(terminated, [true, true]);
            let path = format!(
                "{}/api/locations/{location_id}/targets/development/snapshots",
                first.url
            );
            let current = read_json(&client, (&path, &token.bearer)).await;
            assert_eq!(
                current["record"]["generation"], 1,
                "HTTP still serves during listener recovery"
            );
            expect_topics(&mut a, &["all".into()]).await;
            expect_topics(&mut b, &["all".into()]).await;
            proxy.reset.send_modify(|epoch| *epoch += 1);
            expect_topics(&mut a, &["all".into()]).await;
            expect_topics(&mut b, &["all".into()]).await;
            let current = read_json(&client, (&path, &token.bearer)).await;
            assert_eq!(
                current["record"]["generation"], 1,
                "HTTP survives a TCP reset"
            );
            input.generation = 2;
            publish_snapshot(store, &input)
                .await
                .expect("commit after re-listening");
            let topics = [
                format!("snapshot:{location_id}"),
                "node_status:development".into(),
            ];
            expect_topics(&mut a, &topics).await;
            expect_topics(&mut b, &topics).await;
            let mut reconnected = connect(&client, &second, scope).await;
            let current = read_json(&client, (&path, &token.bearer)).await;
            assert_eq!(
                current["record"]["generation"], test_case.expected_generation,
                "{}",
                test_case.description
            );
            stop_api(first).await;
            stop_api(second).await;
            assert_eq!(next_topic(&mut a).await, None);
            assert_eq!(next_topic(&mut b).await, None);
            assert_eq!(next_topic(&mut reconnected).await, None);
            crate::store::tests::store_helpers::close_store(proxy.store).await;
            proxy.shutdown.send(()).expect("stop database proxy");
            proxy.task.await.expect("database proxy stopped");
            close_test_store(fixture).await;
        });
    }
}
