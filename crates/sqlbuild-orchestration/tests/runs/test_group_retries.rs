//! A member job's `[retry]` reruns it inside its group run while the members that depend on it wait.

use std::collections::BTreeMap;

use serde_json::json;
use uuid::Uuid;

use crate::authorization::main::people::bootstrap_user::bootstrap_user;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::credentials::tests::location_helpers::create_location;
use crate::runs::_helpers::lifecycle::reads::list_runs_in;
use crate::runs::main::{
    execution::advance_groups::advance_groups, execution::claim_next_run::claim_next_run,
    execution::run_execution::run_execution, listings::list_runs::list_runs,
    requests::request_job_run::request_job_run,
};
use crate::runs::models::{JobRunRequest, NodeOutcome, QueueWait, RunOutcome, RunStatus};
use crate::runs::tests::claim_helpers::{finish, wait_out_retry_delay};
use crate::runs::tests::helpers::{rows, snapshot};
use crate::runs::tests::test_types::GroupMemberRetryTestCase;
use crate::runs::types::RunsListing;
use crate::scheduler::tests::contracts::helpers::acquire_lease;
use crate::snapshots::main::publication::publish_snapshot::publish_snapshot;
use crate::store::main::{begin::begin, commit::commit, migrate_store::migrate_store};
use crate::store::models::TransactionMode;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;
use crate::warehouse_history::main::read_person_run_events::read_person_run_events;

#[test]
fn given_a_member_with_retry_when_it_fails_in_a_group_then_it_retries_inside_the_group_while_dependents_wait()
 {
    let test_cases = [
        GroupMemberRetryTestCase {
            description: "SQLite member retry",
            backend: Backend::Sqlite,
            first_outcome: NodeOutcome::Failed,
            delay_seconds: 900,
            expected_messages: &[
                "staging_job failed, retrying in 15 min (attempt 2 of 3)",
                "staging_job succeeded on attempt 2",
            ],
        },
        GroupMemberRetryTestCase {
            description: "PostgreSQL member retry",
            backend: Backend::Postgres,
            first_outcome: NodeOutcome::Failed,
            delay_seconds: 0,
            expected_messages: &[
                "staging_job failed, retrying now (attempt 2 of 3)",
                "staging_job succeeded on attempt 2",
            ],
        },
        GroupMemberRetryTestCase {
            description: "SQLite member retry of a node its stopped worker cancelled",
            backend: Backend::Sqlite,
            first_outcome: NodeOutcome::Cancelled,
            delay_seconds: 0,
            expected_messages: &[
                "staging_job failed, retrying now (attempt 2 of 3)",
                "staging_job succeeded on attempt 2",
            ],
        },
        GroupMemberRetryTestCase {
            description: "PostgreSQL member retry of a node its stopped worker cancelled",
            backend: Backend::Postgres,
            first_outcome: NodeOutcome::Cancelled,
            delay_seconds: 0,
            expected_messages: &[
                "staging_job failed, retrying now (attempt 2 of 3)",
                "staging_job succeeded on attempt 2",
            ],
        },
    ];
    let mut completed = 0;
    for test_case in test_cases {
        run(async {
            let fixture = open_test_store(test_case.backend)
                .await
                .expect("required backend");
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let location = create_location(store, "orders", "test-admin")
                .await
                .expect("location");
            let mut input = snapshot(location);
            input.nodes.get_mut("orders").expect("orders").definition["deps"] =
                json!(["customers"]);
            input.documents.insert("jobs".into(), json!({
                "staging_job": {"description": "Stage customers", "selection": ["customers"],
                    "on_failure": {"retries": 2, "retry_delay_seconds": test_case.delay_seconds}},
                "marts_job": {"description": "Build orders", "selection": ["orders"]}
            }));
            input.documents.insert(
                "groups".into(),
                json!({"nightly_job_group": {
                    "description": "Stage then build orders", "selection": ["customers", "orders"],
                    "jobs": {"staging_job": [], "marts_job": ["staging_job"]}
                }}),
            );
            publish_snapshot(store, &input).await.expect("publish");
            let context = AuditContext {
                actor: "test-operator".into(),
                granting_role: "control-store-owner".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            let group = Uuid::now_v7();
            request_job_run(
                store,
                (&context, None),
                &JobRunRequest {
                    run_id: group,
                    location_id: location,
                    target: "development".into(),
                    job_key: "nightly_job_group".into(),
                    reason: "Build the nightly group".into(),
                    priority: 0,
                    window: None,
                    full_refresh: false,
                    select: Vec::new(),
                    exclude: Vec::new(),
                },
            )
            .await
            .expect("request group");
            let lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("elect")
                .expect("leader");
            let limits = BTreeMap::new();
            let scope = (location, "development");
            advance_groups(store, &lease, (&limits, scope))
                .await
                .expect("start group");
            let first = claim_next_run(store, &lease, &limits, Some(scope))
                .await
                .expect("claim staging")
                .expect("staging ready");
            finish(store, &first, (test_case.first_outcome, RunOutcome::Failed)).await;
            advance_groups(store, &lease, (&limits, scope))
                .await
                .expect("advance after the failure");

            let alerts = rows(
                store,
                "SELECT event_id FROM recall_events WHERE event_type='platform_run_failed'",
                vec![],
            )
            .await;
            assert!(
                alerts.is_empty(),
                "{}: a member that will retry raises no failure alert: {alerts:?}",
                test_case.description
            );
            let transaction = begin(store, TransactionMode::Read).await.expect("begin");
            let (transaction, queued) = list_runs_in(transaction, scope, RunsListing::Queue(20))
                .await
                .expect("queue");
            commit(transaction).await.expect("commit");
            let retry = queued
                .iter()
                .find(|record| record.job_key.as_deref() == Some("staging_job"))
                .expect("queued retry");
            assert_eq!(
                (
                    retry.parent_run_id,
                    retry.rerun_of_run_id,
                    retry.member_attempt
                ),
                (Some(group), Some(first.request.run_id), Some(2)),
                "{}: the retry is another child of the same group run",
                test_case.description
            );
            let marts = queued
                .iter()
                .find(|record| record.job_key.as_deref() == Some("marts_job"))
                .expect("waiting dependent");
            assert_eq!(
                marts.queue.as_ref().expect("queue state").waits,
                vec![QueueWait::UpstreamJob {
                    run_id: retry.run_id,
                    job_key: "staging_job".into(),
                    attempt: Some(2),
                }],
                "{}: the dependent waits for the retrying member instead of skipping",
                test_case.description
            );
            let runs = list_runs(store, scope, 100).await.expect("runs");
            assert_eq!(
                runs.iter()
                    .find(|record| record.run_id == group)
                    .expect("group")
                    .status,
                RunStatus::Running,
                "{}: the group keeps running",
                test_case.description
            );

            wait_out_retry_delay(
                store,
                (&lease, (&limits, scope)),
                (retry.run_id, test_case.delay_seconds),
                test_case.description,
            )
            .await;
            let second = claim_next_run(store, &lease, &limits, Some(scope))
                .await
                .expect("claim retry")
                .expect("retry ready");
            assert_eq!(second.request.run_id, retry.run_id);
            assert_eq!(second.request.selection, ["customers".to_owned()].into());
            finish(store, &second, (NodeOutcome::Success, RunOutcome::Success)).await;
            advance_groups(store, &lease, (&limits, scope))
                .await
                .expect("advance after the retry");
            let dependent = claim_next_run(store, &lease, &limits, Some(scope))
                .await
                .expect("claim dependent")
                .expect("dependent ready");
            assert_eq!(dependent.request.job_key.as_deref(), Some("marts_job"));
            assert!(
                run_execution(store, (dependent.request.run_id, dependent.claim_id))
                    .await
                    .expect("execution")
                    .expect("claimed")
                    .skipped_nodes
                    .is_empty(),
                "{}: the dependent builds everything once the retry succeeded",
                test_case.description
            );
            finish(
                store,
                &dependent,
                (NodeOutcome::Success, RunOutcome::Success),
            )
            .await;
            for _ in 0..2 {
                advance_groups(store, &lease, (&limits, scope))
                    .await
                    .expect("finish group");
            }
            let runs = list_runs(store, scope, 100).await.expect("runs");
            assert_eq!(
                runs.iter()
                    .find(|record| record.run_id == group)
                    .expect("group")
                    .status,
                RunStatus::Success,
                "{}: the group succeeds on the member's final outcome",
                test_case.description
            );
            let admin = bootstrap_user(store, &context, "orders-admin")
                .await
                .expect("administrator");
            let events = read_person_run_events(
                store,
                (&context, admin),
                ((location, "development", group), (0, 1000)),
                DefaultRole::None,
            )
            .await
            .expect("group events")
            .expect("group run");
            let messages: Vec<&str> = events
                .iter()
                .filter_map(|event| event.message.as_deref())
                .collect();
            assert_eq!(
                messages, test_case.expected_messages,
                "{}",
                test_case.description
            );
            completed += 1;
            close_test_store(fixture).await;
        });
    }
    assert_eq!(completed, 4, "both backends run");
}
