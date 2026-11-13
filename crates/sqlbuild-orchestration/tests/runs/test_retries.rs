//! A job's `on_failure` policy reruns failed work from failure until its retries are spent or a person cancels.

use std::collections::BTreeMap;

use serde_json::json;
use uuid::Uuid;

use crate::credentials::models::AuditContext;
use crate::credentials::tests::location_helpers::create_location;
use crate::runs::_helpers::lifecycle::reads::list_runs_in;
use crate::runs::main::{
    change_run::change_run, claim_next_run::claim_next_run, list_runs::list_runs,
    request_job_run::request_job_run, request_rerun::request_rerun,
};
use crate::runs::models::{
    JobRunRequest, NodeOutcome, QueueWait, RerunRequest, RunAuthority, RunChange, RunChangeRequest,
    RunOrigin, RunOutcome, RunStatus,
};
use crate::runs::tests::claim_helpers::finish;
use crate::runs::tests::helpers::snapshot;
use crate::runs::tests::test_types::RetryChainTestCase;
use crate::runs::types::RunsListing;
use crate::scheduler::tests::contracts::helpers::acquire_lease;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::store::main::{begin::begin, commit::commit, migrate_store::migrate_store};
use crate::store::models::TransactionMode;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_a_job_with_on_failure_when_its_runs_fail_then_retries_rerun_failed_work_until_spent_or_cancelled()
 {
    let test_cases = [
        RetryChainTestCase {
            description: "SQLite retry chain",
            backend: Backend::Sqlite,
            expected_attempts: vec![1, 2],
            expected_delayed_attempt: 1,
        },
        RetryChainTestCase {
            description: "PostgreSQL retry chain",
            backend: Backend::Postgres,
            expected_attempts: vec![1, 2],
            expected_delayed_attempt: 1,
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
            let context = AuditContext {
                actor: "test-operator".into(),
                granting_role: "control-store-owner".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            let mut input = snapshot(location);
            input.documents.insert(
                "jobs".into(),
                json!({
                    "orders": {"description": "Build orders", "selection": ["orders", "customers"],
                        "on_failure": {"retries": 2, "retry_delay_seconds": 0}},
                    "inventory": {"description": "Build inventory", "selection": ["inventory"],
                        "on_failure": {"retries": 3, "retry_delay_seconds": 3600}}
                }),
            );
            publish_snapshot(store, &input).await.expect("publish");
            let lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("elect")
                .expect("leader");
            let limits = BTreeMap::new();
            let scope = (location, "development");
            let requested = |job_key: &str| JobRunRequest {
                run_id: Uuid::now_v7(),
                location_id: location,
                target: "development".into(),
                job_key: job_key.into(),
                reason: "Build".into(),
                priority: 0,
                window: None,
                full_refresh: false,
                select: Vec::new(),
                exclude: Vec::new(),
            };
            let source = requested("orders");
            request_job_run(store, (&context, None), &source)
                .await
                .expect("request orders");
            let first = claim_next_run(store, &lease, &limits, Some(scope))
                .await
                .expect("claim")
                .expect("source run");
            finish(store, &first, (NodeOutcome::Failed, RunOutcome::Failed)).await;
            let retry_one = claim_next_run(store, &lease, &limits, Some(scope))
                .await
                .expect("claim first retry")
                .expect("first retry");
            finish(store, &retry_one, (NodeOutcome::Failed, RunOutcome::Failed)).await;
            let retry_two = claim_next_run(store, &lease, &limits, Some(scope))
                .await
                .expect("claim second retry")
                .expect("second retry");
            finish(store, &retry_two, (NodeOutcome::Failed, RunOutcome::Failed)).await;
            assert!(
                claim_next_run(store, &lease, &limits, Some(scope))
                    .await
                    .expect("claim after retries are spent")
                    .is_none(),
                "{}: no retry is queued once retries are spent",
                test_case.description
            );
            assert_eq!(
                [&retry_one, &retry_two]
                    .map(|claim| (claim.request.origin, claim.request.rerun_of_run_id)),
                [
                    (RunOrigin::Rerun, Some(source.run_id)),
                    (RunOrigin::Rerun, Some(retry_one.request.run_id)),
                ],
                "{}: each retry reruns the run before it",
                test_case.description
            );
            assert_eq!(
                [&retry_one, &retry_two]
                    .map(|claim| claim.request.retry.as_ref().map(|retry| retry.attempt))
                    .to_vec(),
                test_case
                    .expected_attempts
                    .iter()
                    .copied()
                    .map(Some)
                    .collect::<Vec<_>>(),
                "{}",
                test_case.description
            );

            let delayed = requested("inventory");
            request_job_run(store, (&context, None), &delayed)
                .await
                .expect("request inventory");
            let claim = claim_next_run(store, &lease, &limits, Some(scope))
                .await
                .expect("claim")
                .expect("inventory run");
            finish(store, &claim, (NodeOutcome::Failed, RunOutcome::Failed)).await;
            assert!(
                claim_next_run(store, &lease, &limits, Some(scope))
                    .await
                    .expect("claim before the delay")
                    .is_none(),
                "a retry is not claimed before its delay"
            );
            let transaction = begin(store, TransactionMode::Read).await.expect("begin");
            let (transaction, queued) = list_runs_in(transaction, scope, RunsListing::Queue(20))
                .await
                .expect("queue");
            commit(transaction).await.expect("commit");
            let [waiting] = queued.as_slice() else {
                panic!("one waiting retry, not {queued:?}");
            };
            let waits = &waiting.queue.as_ref().expect("queue state").waits;
            assert!(
                matches!(waits.as_slice(), [QueueWait::RetryDelay { attempt, .. }] if *attempt == test_case.expected_delayed_attempt),
                "{}: {waits:?}",
                test_case.description
            );
            change_run(
                store,
                RunAuthority::Operator(&context),
                &RunChangeRequest {
                    event_id: Uuid::now_v7(),
                    run_id: waiting.run_id,
                    claim_id: None,
                    change: RunChange::CancelRequested {
                        actor: "test-operator".into(),
                        reason: "stop retrying".into(),
                    },
                },
            )
            .await
            .expect("cancel the pending retry");
            let runs = list_runs(store, scope, 20).await.expect("list");
            let inventory: Vec<RunStatus> = runs
                .iter()
                .filter(|run| run.job_key.as_deref() == Some("inventory"))
                .map(|run| run.status)
                .collect();
            assert_eq!(
                inventory,
                vec![RunStatus::Cancelled, RunStatus::Failed],
                "{}: cancelling the pending retry ends the chain",
                test_case.description
            );
            let rerun = RerunRequest {
                run_id: Uuid::now_v7(),
                source_run_id: waiting.run_id,
                from_failure: false,
                reason: "Retry by hand".into(),
                priority: 0,
                nodes: Vec::new(),
            };
            request_rerun(store, (&context, None), &rerun)
                .await
                .expect("rerun the cancelled retry");
            assert_eq!(
                claim_next_run(store, &lease, &limits, Some(scope))
                    .await
                    .expect("claim the rerun")
                    .map(|claim| claim.request.run_id),
                Some(rerun.run_id),
                "{}: a person's rerun of a retry starts at once, without the retry's delay",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}
