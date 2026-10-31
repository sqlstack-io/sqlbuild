//! Privileged test operators stay separate from the API-only worker session.

use std::collections::BTreeMap;
use std::time::{Duration, SystemTime};

use sqlbuild_orchestration::credentials::main::create_location::create_location;
use sqlbuild_orchestration::credentials::main::issue_token::issue_token;
use sqlbuild_orchestration::credentials::main::revoke_token::revoke_token;
use sqlbuild_orchestration::credentials::models::{AuditContext, CredentialScope};
use sqlbuild_orchestration::runs::main::change_run::change_run;
use sqlbuild_orchestration::runs::main::claim_next_run::claim_next_run;
use sqlbuild_orchestration::runs::main::request_run::request_run;
use sqlbuild_orchestration::runs::models::{
    RunAuthority, RunChange, RunChangeRequest, RunKind, RunOrigin, RunRequest,
};
use sqlbuild_orchestration::scheduler::main::acquire_lease::acquire_lease;
use sqlbuild_orchestration::scheduler::models::Overlap;
use sqlbuild_orchestration::snapshots::main::publish_snapshot::publish_snapshot;
use sqlbuild_orchestration::snapshots::models::{SnapshotInput, SnapshotNode};
use sqlbuild_orchestration::store::main::open_store::open_store;
use sqlbuild_orchestration::worker::main::connect_worker::connect_worker;
use sqlbuild_orchestration::worker::main::start_worker::start_worker;
use sqlbuild_orchestration::worker::models::WorkerSession;
use uuid::Uuid;

use crate::test_types::TestControl;
use crate::worker::test_types::WorkerControl;

pub(crate) async fn start_claimed_worker(
    control: &TestControl,
    url: &str,
) -> (WorkerSession, WorkerControl) {
    let path = control.directory.path().join(".sqlbuild/control.sqlite3");
    let store = open_store(path.to_str().expect("test database path"))
        .await
        .expect("native control store");
    let context = AuditContext {
        actor: "test-admin".into(),
        granting_role: "admin".into(),
        request_id: Uuid::now_v7(),
        ip_address: None,
    };
    let location_id = create_location(&store, "orders", &context)
        .await
        .expect("location");
    let baseline = SnapshotInput {
        location_id,
        target: "development".into(),
        generation: 1,
        git_revision: None,
        git_dirty: false,
        git_author: None,
        image_id: None,
        environment_hash: "orders-environment".into(),
        sqb_version: env!("CARGO_PKG_VERSION").into(),
        nodes: BTreeMap::from([(
            "orders".into(),
            SnapshotNode {
                kind: "model".into(),
                definition: serde_json::json!({"materialization": "table"}),
                raw_sql: Some("select 1 as id".into()),
                compiled_sql: Some("select 1 as id".into()),
                python_source: None,
            },
        )]),
        documents: BTreeMap::new(),
    };
    let publication = publish_snapshot(&store, &baseline).await.expect("snapshot");
    let run_id = Uuid::now_v7();
    let request = RunRequest {
        run_id,
        location_id,
        target: "development".into(),
        snapshot_id: publication.snapshot_id,
        image_id: None,
        kind: RunKind::Adhoc,
        origin: RunOrigin::Manual,
        job_key: None,
        parent_run_id: None,
        root_run_id: run_id,
        trigger_id: None,
        tick_id: None,
        rerun_of_run_id: None,
        requested_by: "test-admin".into(),
        priority: 0,
        selection: ["orders".into()].into(),
        write_set: ["orders".into()].into(),
        settings: BTreeMap::new(),
        concurrency_groups: Default::default(),
        overlap: Overlap::Allow,
    };
    request_run(&store, &request).await.expect("queue run");
    let lease = acquire_lease(&store, Uuid::now_v7())
        .await
        .expect("acquire lease")
        .expect("leader");
    let claim = claim_next_run(&store, &lease, &BTreeMap::new())
        .await
        .expect("claim")
        .expect("queued work");
    let identity = (run_id, claim.claim_id);
    let token = issue_token(
        &store,
        &context,
        CredentialScope::Run {
            run_id,
            claim_id: identity.1,
        },
        (SystemTime::now() + Duration::from_secs(3600)).into(),
    )
    .await
    .expect("run-scoped token");
    change_run(
        &store,
        RunAuthority::Scheduler(&lease),
        &RunChangeRequest {
            event_id: Uuid::now_v7(),
            run_id,
            claim_id: Some(identity.1),
            change: RunChange::Launched { container_id: None },
        },
    )
    .await
    .expect("launched worker");
    let client = connect_worker(url, &token.bearer, identity).expect("worker connection");
    let session = start_worker(&client, &baseline)
        .await
        .expect("verified worker");
    (
        session,
        WorkerControl {
            store,
            context,
            token_id: token.identity.token_id,
            identity,
        },
    )
}

pub(crate) fn cancel_worker(worker: WorkerControl, _control: &mut TestControl) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("independent operator runtime");
    runtime
        .block_on(change_run(
            &worker.store,
            RunAuthority::Operator("test-admin"),
            &RunChangeRequest {
                event_id: Uuid::now_v7(),
                run_id: worker.identity.0,
                claim_id: Some(worker.identity.1),
                change: RunChange::CancelRequested {
                    actor: "test-admin".into(),
                },
            },
        ))
        .expect("request cancellation while worker runtime is blocked");
}

pub(crate) fn revoke_worker(worker: WorkerControl, _control: &mut TestControl) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("independent operator runtime");
    assert!(
        runtime
            .block_on(revoke_token(
                &worker.store,
                &worker.context,
                worker.token_id
            ))
            .expect("revoke worker credential")
    );
}

pub(crate) fn disconnect_worker(_worker: WorkerControl, control: &mut TestControl) {
    control.child.kill().expect("stop API process");
    control.child.wait().expect("reap API process");
}
