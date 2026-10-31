//! Read group membership and failure policy from the parent's immutable publication.

use std::collections::BTreeSet;

use uuid::Uuid;

use crate::declarations::main::frozen_definitions::frozen_definitions;
use crate::declarations::models::OnFailure;
use crate::runs::models::{GroupContext, GroupRun, RunStatus};
use crate::snapshots::main::read_snapshot_nodes::read_snapshot_nodes;
use crate::store::errors::ControlError;
use crate::store::main::{decode_row::decode_row, query::query};
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn load_group(
    transaction: ControlTransaction,
    parent_id: Uuid,
) -> Result<(ControlTransaction, GroupContext), ControlError> {
    let (transaction, rows) = query(transaction,
        "SELECT r.request_json, r.queued_at, r.status, r.cancel_requested_at, r.node_counts_json, l.worker_instance FROM sqb_runs r \
         LEFT JOIN sqb_run_liveness l ON l.run_id=r.run_id WHERE r.run_id=$1 AND r.kind='group'",
        vec![Parameter::Id(parent_id)]).await?;
    let parent: GroupRun = decode_row(
        rows.into_iter()
            .next()
            .ok_or_else(|| ControlError::Invalid("group run is missing".into()))?,
    )?;
    let (transaction, rows) = query(transaction,
        "SELECT r.request_json, r.queued_at, r.status, r.cancel_requested_at, r.node_counts_json, l.worker_instance FROM sqb_runs r \
         LEFT JOIN sqb_run_liveness l ON l.run_id=r.run_id WHERE r.parent_run_id=$1 ORDER BY r.run_id",
        vec![Parameter::Id(parent_id)]).await?;
    let children = rows.into_iter().map(decode_row).collect::<Result<_, _>>()?;
    let (transaction, definitions) =
        frozen_definitions(transaction, parent.request_json.snapshot_id).await?;
    let (transaction, failures) = query(
        transaction,
        "SELECT n.node_key FROM sqb_run_nodes n JOIN sqb_runs r ON r.run_id=n.run_id \
         WHERE r.parent_run_id=$1 AND n.status='failed'",
        vec![Parameter::Id(parent_id)],
    )
    .await?;
    let failures: BTreeSet<String> = failures
        .iter()
        .filter_map(|row| row["node_key"].as_str().map(str::to_owned))
        .collect();
    let (transaction, nodes) = read_snapshot_nodes(
        transaction,
        parent.request_json.snapshot_id,
        Some(&failures),
    )
    .await?;
    let critical_failed = nodes
        .values()
        .any(|node| node.definition["config"]["critical"].as_bool() == Some(true));
    Ok((
        transaction,
        GroupContext {
            parent,
            children,
            definitions,
            critical_failed,
        },
    ))
}

impl GroupContext {
    pub(crate) fn stopped(&self) -> bool {
        self.critical_failed
            || self.children.iter().any(|child| {
                child.request_json.job_key.as_ref().is_some_and(|name| {
                    self.definitions.jobs[name].on_failure == OnFailure::StopGroup
                        && (child
                            .state
                            .node_counts_json
                            .get("failed")
                            .copied()
                            .unwrap_or_default()
                            > 0
                            || matches!(
                                child.state.status,
                                RunStatus::Failed | RunStatus::PresumedFailed
                            ))
                })
            })
    }
}
