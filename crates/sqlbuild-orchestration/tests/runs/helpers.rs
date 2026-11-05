//! Run fixtures exercise native queues and Recall against isolated real databases.

use std::collections::{BTreeMap, BTreeSet};

use crate::runs::_helpers::requests::publication::publish_request;
use crate::runs::models::{RunKind, RunOrigin, RunRequest};
use crate::snapshots::models::{SnapshotInput, SnapshotNode};
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::models::{ControlStore, TransactionMode};
use serde_json::json;
use uuid::Uuid;

pub(crate) async fn request_run(
    store: &ControlStore,
    request: &RunRequest,
) -> Result<bool, ControlError> {
    let transaction = begin(store, TransactionMode::Write).await?;
    let (transaction, applied) = publish_request(transaction, request).await?;
    commit(transaction).await?;
    Ok(applied)
}

pub(crate) fn snapshot(location_id: Uuid) -> SnapshotInput {
    SnapshotInput {
        location_id,
        target: "development".into(),
        generation: 1,
        git_revision: None,
        git_dirty: false,
        git_author: None,
        image_id: Some("sha256:synthetic-orders-image".into()),
        environment_hash: "orders-environment".into(),
        sqb_version: env!("CARGO_PKG_VERSION").into(),
        nodes: ["customers", "inventory", "orders", "products"]
            .into_iter()
            .map(|name| {
                (
                    name.into(),
                    SnapshotNode {
                        kind: "model".into(),
                        definition: json!({"materialization": "table"}),
                        raw_sql: Some("select 1 as id".into()),
                        compiled_sql: Some("select 1 as id".into()),
                        python_source: None,
                    },
                )
            })
            .collect(),
        documents: BTreeMap::new(),
    }
}

pub(crate) fn request(
    (location_id, snapshot_id): (Uuid, Uuid),
    id: u128,
    priority: i64,
    nodes: &[&str],
) -> RunRequest {
    let selection: BTreeSet<String> = nodes.iter().map(|name| (*name).into()).collect();
    let run_id = Uuid::from_u128(id);
    RunRequest {
        run_id,
        location_id,
        target: "development".into(),
        snapshot_id,
        image_id: Some("sha256:synthetic-orders-image".into()),
        kind: RunKind::Adhoc,
        origin: RunOrigin::Manual,
        job_key: None,
        parent_run_id: None,
        root_run_id: run_id,
        trigger_id: None,
        tick_id: None,
        rerun_of_run_id: None,
        requested_by: "test-operator".into(),
        priority,
        write_set: selection.clone(),
        selection,
        settings: BTreeMap::new(),
        window: None,
        concurrency_groups: BTreeSet::new(),
    }
}

pub(crate) async fn queue(store: &ControlStore, request: &RunRequest) {
    assert!(
        request_run(store, request)
            .await
            .expect("queue pinned request")
    );
    assert!(
        !request_run(store, request)
            .await
            .expect("retry pinned request")
    );
    let mut changed = request.clone();
    changed.priority += 1;
    assert!(
        request_run(store, &changed).await.is_err(),
        "a reused run ID cannot change its request"
    );
}
