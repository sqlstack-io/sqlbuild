//! Privileged test operators stay separate from the API-only worker session.

use std::collections::BTreeMap;
use std::time::{Duration, SystemTime};

use sqlbuild_orchestration::credentials::main::create_location::create_location;
use sqlbuild_orchestration::credentials::main::issue_token::issue_token;
use sqlbuild_orchestration::credentials::main::revoke_token::revoke_token;
use sqlbuild_orchestration::credentials::models::{AuditContext, CredentialScope};
use sqlbuild_orchestration::runs::main::change_run::change_run;
use sqlbuild_orchestration::runs::main::claim_next_run::claim_next_run;
use sqlbuild_orchestration::runs::models::{RunAuthority, RunChange, RunChangeRequest};
use sqlbuild_orchestration::scheduler::main::acquire_lease::acquire_lease;
use sqlbuild_orchestration::snapshots::main::publish_snapshot::publish_snapshot;
use sqlbuild_orchestration::snapshots::models::{SnapshotInput, SnapshotNode};
use sqlbuild_orchestration::store::main::open_store::open_store;
use sqlbuild_orchestration::worker::main::connect_worker::connect_worker;
use sqlbuild_orchestration::worker::main::start_worker::start_worker;
use sqlbuild_orchestration::worker::models::WorkerSession;
use uuid::Uuid;

use crate::admin_helpers::administer;
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
        documents: BTreeMap::from([(
            "jobs".into(),
            serde_json::json!({"daily_orders": {
                "description": "Build orders", "selection": ["orders"], "overlap": "allow", "settings": {"full_refresh": true}
            }}),
        )]),
    };
    publish_snapshot(&store, &baseline).await.expect("snapshot");
    let run_id = Uuid::now_v7();
    let location = location_id.to_string();
    let run = run_id.to_string();
    let arguments = [
        "request-run",
        &location,
        "daily_orders",
        "--target",
        "development",
        "--run-id",
        &run,
        "--priority",
        "-3",
        "--reason",
        "Refresh orders",
    ];
    let requested = administer(control, &arguments);
    assert_eq!(requested["run_id"], run);
    assert_eq!(requested["applied"], true);
    assert_eq!(administer(control, &arguments)["applied"], false);
    let connection = rusqlite::Connection::open(&path).expect("audit connection");
    let count: i64 = connection.query_row(
        "SELECT COUNT(*) FROM sqb_security_audit WHERE action='job.run_requested' AND actor='local-control-store-owner' AND granting_role='control-store-owner' AND json_extract(scope_json,'$.run_id')=?1",
        [&run], |row| row.get(0)).expect("trusted manual request audit");
    assert_eq!(count, 1);
    drop(connection);
    let lease = acquire_lease(&store, Uuid::now_v7())
        .await
        .expect("acquire lease")
        .expect("leader");
    let claim = claim_next_run(
        &store,
        &lease,
        &BTreeMap::new(),
        Some((location_id, "development")),
    )
    .await
    .expect("claim")
    .expect("queued work");
    assert_eq!(claim.request.priority, -3);
    assert_eq!(
        claim.request.settings.get("full_refresh"),
        Some(&serde_json::json!(true))
    );
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

pub(crate) fn cancel_worker(worker: WorkerControl, control: &mut TestControl) {
    let event_id = Uuid::now_v7().to_string();
    let run_id = worker.identity.0.to_string();
    let arguments = [
        "cancel-run",
        &run_id,
        "--event-id",
        &event_id,
        "--reason",
        "Stop the orders build",
    ];
    let applied = administer(control, &arguments);
    assert_eq!(applied["run_id"], run_id);
    assert_eq!(applied["event_id"], event_id);
    assert_eq!(applied["applied"], true);
    assert_eq!(administer(control, &arguments)["applied"], false);
    let connection =
        rusqlite::Connection::open(control.directory.path().join(".sqlbuild/control.sqlite3"))
            .expect("observe durable cancellation audit");
    connection
        .query_row(
            "SELECT actor, granting_role, scope_json, before_json, after_json FROM sqb_security_audit WHERE event_id=?1 AND action='run.cancel_requested'",
            [&event_id],
            |row| {
                let actor: String = row.get(0)?;
                let role: String = row.get(1)?;
                let scope: String = row.get(2)?;
                let before: String = row.get(3)?;
                let after: String = row.get(4)?;
                let scope: serde_json::Value = serde_json::from_str(&scope).expect("audit scope");
                let before: serde_json::Value = serde_json::from_str(&before).expect("before state");
                let after: serde_json::Value = serde_json::from_str(&after).expect("after state");
                assert_eq!(actor, "local-control-store-owner");
                assert_eq!(role, "control-store-owner");
                assert_eq!(scope["run_id"], run_id);
                assert_eq!(scope["target"], "development");
                assert!(scope["location_id"].is_string());
                assert_eq!(before["status"], "running");
                assert!(before["cancel_requested_at"].is_null());
                assert_eq!(after["status"], "running");
                assert!(after["cancel_requested_at"].is_string());
                assert_eq!(after["reason"], "Stop the orders build");
                Ok(())
            },
        )
        .expect("same-transaction operator history");
    let claims: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqb_write_claims WHERE run_id=?1",
            [&run_id],
            |row| row.get(0),
        )
        .expect("physical claims after cancellation request");
    assert_eq!(
        claims, 1,
        "cancellation is not proof of physical termination"
    );
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
