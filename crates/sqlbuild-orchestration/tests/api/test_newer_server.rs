//! A worker refuses a run request with any part its release can't read, an unknown field or variant at any depth, failing the run with why, rather than run part of what was asked.

use std::collections::BTreeMap;

use chrono::Utc;
use uuid::Uuid;

use crate::api::tests::helpers::{start_api, stop_api};
use crate::api::tests::newer_server_helpers::{newer_server, refusal};
use crate::api::tests::test_types::NewerServerTestCase;
use crate::credentials::main::tokens::issue_token::issue_token;
use crate::credentials::models::{AuditContext, CredentialScope};
use crate::credentials::tests::location_helpers::create_location;
use crate::runs::main::execution::change_run::change_run;
use crate::runs::main::execution::claim_next_run::claim_next_run;
use crate::runs::main::listings::list_runs::list_runs;
use crate::runs::models::{RunAuthority, RunChange, RunChangeRequest, RunStatus};
use crate::runs::tests::helpers::{queue, request, snapshot};
use crate::scheduler::tests::contracts::helpers::{acquire_lease, hold_lease};
use crate::snapshots::main::publication::publish_snapshot::publish_snapshot;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;
use crate::worker::main::connect_worker::connect_worker;
use crate::worker::main::start_worker::start_worker;

#[test]
fn given_a_newer_server_when_a_worker_fetches_its_execution_then_an_unreadable_part_fails_the_run()
{
    let test_cases = [
        NewerServerTestCase {
            description: "SQLite: an execution of the current shape starts",
            backend: Backend::Sqlite,
            edit: None,
            expected_status: RunStatus::Running,
            expected_unreadable: None,
        },
        NewerServerTestCase {
            description: "SQLite: an unknown top-level field fails the run",
            backend: Backend::Sqlite,
            edit: Some((&["added_by_newer_server"], "true")),
            expected_status: RunStatus::Failed,
            expected_unreadable: Some("unknown field `added_by_newer_server`"),
        },
        NewerServerTestCase {
            description: "SQLite: an unknown field in the request fails the run",
            backend: Backend::Sqlite,
            edit: Some((&["claim", "request", "added_by_newer_server"], "true")),
            expected_status: RunStatus::Failed,
            expected_unreadable: Some("unknown field `claim.request.added_by_newer_server`"),
        },
        NewerServerTestCase {
            description: "SQLite: an unknown command fails the run",
            backend: Backend::Sqlite,
            edit: Some((
                &["claim", "request", "command"],
                r#""added_by_newer_server""#,
            )),
            expected_status: RunStatus::Failed,
            expected_unreadable: Some(
                "unknown variant `added_by_newer_server` in `claim.request.command`",
            ),
        },
        NewerServerTestCase {
            description: "SQLite: an unknown field in the window fails the run",
            backend: Backend::Sqlite,
            edit: Some((
                &["claim", "request", "window"],
                r#"{"kind": "integer", "start": 1, "end": 2, "added_by_newer_server": true}"#,
            )),
            expected_status: RunStatus::Failed,
            expected_unreadable: Some(
                "unknown field `added_by_newer_server` in `claim.request.window`",
            ),
        },
        NewerServerTestCase {
            description: "PostgreSQL: an execution of the current shape starts",
            backend: Backend::Postgres,
            edit: None,
            expected_status: RunStatus::Running,
            expected_unreadable: None,
        },
        NewerServerTestCase {
            description: "PostgreSQL: an unknown top-level field fails the run",
            backend: Backend::Postgres,
            edit: Some((&["added_by_newer_server"], "true")),
            expected_status: RunStatus::Failed,
            expected_unreadable: Some("unknown field `added_by_newer_server`"),
        },
        NewerServerTestCase {
            description: "PostgreSQL: an unknown field in the request fails the run",
            backend: Backend::Postgres,
            edit: Some((&["claim", "request", "added_by_newer_server"], "true")),
            expected_status: RunStatus::Failed,
            expected_unreadable: Some("unknown field `claim.request.added_by_newer_server`"),
        },
        NewerServerTestCase {
            description: "PostgreSQL: an unknown command fails the run",
            backend: Backend::Postgres,
            edit: Some((
                &["claim", "request", "command"],
                r#""added_by_newer_server""#,
            )),
            expected_status: RunStatus::Failed,
            expected_unreadable: Some(
                "unknown variant `added_by_newer_server` in `claim.request.command`",
            ),
        },
        NewerServerTestCase {
            description: "PostgreSQL: an unknown field in the window fails the run",
            backend: Backend::Postgres,
            edit: Some((
                &["claim", "request", "window"],
                r#"{"kind": "integer", "start": 1, "end": 2, "added_by_newer_server": true}"#,
            )),
            expected_status: RunStatus::Failed,
            expected_unreadable: Some(
                "unknown field `added_by_newer_server` in `claim.request.window`",
            ),
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
            let lease = acquire_lease(store, (Uuid::now_v7(), None))
                .await
                .expect("acquire")
                .expect("leader");
            let holder = hold_lease(store, &lease);
            let claim = claim_next_run(store, &lease, &BTreeMap::new(), None)
                .await
                .expect("claim")
                .expect("work");
            let identity = (claim.request.run_id, claim.claim_id);
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
            let newer = newer_server(&api.url, test_case.edit).await;
            let worker =
                connect_worker(&newer.url, &token.bearer, identity).expect("worker client");

            let started = start_worker(&worker, &baseline).await;
            let runs = list_runs(store, (location, "development"), 10)
                .await
                .expect("runs");

            let expected_reason: Option<String> = test_case
                .expected_unreadable
                .map(|unreadable| refusal(&baseline.sqb_version, unreadable));
            assert_eq!(
                (
                    started.as_ref().err().map(ToString::to_string),
                    runs[0].status,
                    runs[0].status_reason.clone()
                ),
                (
                    expected_reason.clone(),
                    test_case.expected_status,
                    expected_reason
                ),
                "{}",
                test_case.description
            );
            drop(started);
            stop_api(newer).await;
            stop_api(api).await;
            holder.abort();
            close_test_store(fixture).await;
        });
    }
}
