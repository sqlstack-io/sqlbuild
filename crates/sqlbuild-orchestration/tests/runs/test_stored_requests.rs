//! Run requests stored before 0.245 still carry `concurrency_groups`; the server reads them leniently, so such runs still claim, retry, rerun and cancel.

use std::collections::BTreeMap;

use serde_json::json;
use uuid::Uuid;

use crate::credentials::tests::role_helpers::{credentials, role_fixture};
use crate::runs::main::execution::claim_next_run::claim_next_run;
use crate::runs::main::listings::list_runs::list_runs;
use crate::runs::main::requests::request_job_run::request_job_run;
use crate::runs::main::requests::request_person_run::request_person_run;
use crate::runs::main::requests::request_rerun::request_rerun;
use crate::runs::models::{
    CancelRunRequest, JobRunRequest, NodeOutcome, PersonRunRequest, RerunRequest, RunOrigin,
    RunOutcome, RunStatus,
};
use crate::runs::tests::claim_helpers::finish;
use crate::runs::tests::helpers::{age_stored_requests, snapshot};
use crate::runs::tests::test_types::StoredRequestTestCase;
use crate::scheduler::tests::contracts::helpers::acquire_lease;
use crate::snapshots::main::publication::publish_snapshot::publish_snapshot;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_requests_stored_before_0_245_when_the_server_reads_them_then_runs_claim_retry_rerun_and_cancel()
 {
    let test_cases = [
        StoredRequestTestCase {
            description: "SQLite store upgraded from before 0.245",
            backend: Backend::Sqlite,
            expected_outcomes: (
                true,
                RunOrigin::Rerun,
                RunStatus::Queued,
                RunStatus::Cancelled,
            ),
        },
        StoredRequestTestCase {
            description: "PostgreSQL store upgraded from before 0.245",
            backend: Backend::Postgres,
            expected_outcomes: (
                true,
                RunOrigin::Rerun,
                RunStatus::Queued,
                RunStatus::Cancelled,
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
            let roles = role_fixture(store).await;
            let location = roles.location_id;
            let mut input = snapshot(location);
            input.generation = 2;
            input.documents.insert(
                "jobs".into(),
                json!({"orders": {"description": "Build orders", "selection": ["orders", "customers"],
                    "on_failure": {"retries": 1, "retry_delay_seconds": 0}}}),
            );
            publish_snapshot(store, &input).await.expect("publish");
            let lease = acquire_lease(store, (Uuid::now_v7(), None))
                .await
                .expect("elect")
                .expect("leader");
            let limits = BTreeMap::new();
            let scope = (location, "development");
            let source = JobRunRequest {
                run_id: Uuid::now_v7(),
                location_id: location,
                target: "development".into(),
                job_key: "orders".into(),
                reason: "Build".into(),
                priority: 0,
                window: None,
                full_refresh: false,
                select: Vec::new(),
                exclude: Vec::new(),
            };
            request_job_run(store, (&roles.context, None), &source)
                .await
                .expect("request orders");
            age_stored_requests(store).await;

            let first = claim_next_run(store, &lease, &limits, Some(scope))
                .await
                .expect("claim a stored request")
                .expect("the source run");
            finish(store, &first, (NodeOutcome::Failed, RunOutcome::Failed)).await;
            age_stored_requests(store).await;
            let retry = claim_next_run(store, &lease, &limits, Some(scope))
                .await
                .expect("claim the retry of a stored request")
                .expect("the retry");
            finish(store, &retry, (NodeOutcome::Failed, RunOutcome::Failed)).await;
            age_stored_requests(store).await;

            let rerun = RerunRequest {
                run_id: Uuid::now_v7(),
                source_run_id: source.run_id,
                from_failure: true,
                reason: "Rerun by hand".into(),
                priority: 0,
                nodes: Vec::new(),
            };
            request_rerun(store, (&roles.context, None), &rerun)
                .await
                .expect("rerun a stored request");
            let status = |runs: &[crate::runs::models::RunRecord]| {
                runs.iter()
                    .find(|record| record.run_id == rerun.run_id)
                    .map(|record| record.status)
            };
            let rerun_status = status(&list_runs(store, scope, 20).await.expect("list"));
            age_stored_requests(store).await;
            request_person_run(
                store,
                (&roles.context, credentials(&roles.administrator)),
                PersonRunRequest::Cancel(&CancelRunRequest {
                    run_id: rerun.run_id,
                    event_id: Uuid::now_v7(),
                    reason: "Stop the rerun".into(),
                }),
            )
            .await
            .expect("cancel a stored request");
            let cancelled = status(&list_runs(store, scope, 20).await.expect("list"));

            assert_eq!(
                (
                    first.request.run_id == source.run_id,
                    retry.request.origin,
                    rerun_status.expect("the rerun is listed"),
                    cancelled.expect("the cancelled rerun is listed"),
                ),
                test_case.expected_outcomes,
                "{}",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}
