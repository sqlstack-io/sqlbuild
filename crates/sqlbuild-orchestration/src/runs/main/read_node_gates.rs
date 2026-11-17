//! A claimed group member asks which of its gated nodes may run now.

use uuid::Uuid;

use crate::runs::_helpers::groups::gate::node_gates;
use crate::runs::models::{ExecutionMetadata, NodeGates, NodeGatesRequest};
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::decode_row::decode_row;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter, TransactionMode};

/// Each asked-about node's gate, or `None` once the claim is no longer active.
pub async fn read_node_gates(
    store: &ControlStore,
    (run_id, claim_id): (Uuid, Uuid),
    asked: &NodeGatesRequest,
) -> Result<Option<NodeGates>, ControlError> {
    let transaction = begin(store, TransactionMode::Read).await?;
    let (transaction, rows) = query(
        transaction,
        "SELECT r.request_json, s.environment_hash, s.sqb_version FROM sqb_runs r \
         JOIN sqb_snapshots s ON s.snapshot_id=r.snapshot_id \
         WHERE r.run_id=$1 AND r.worker_instance=$2",
        vec![Parameter::Id(run_id), Parameter::Id(claim_id)],
    )
    .await?;
    let Some(row) = rows.into_iter().next() else {
        commit(transaction).await?;
        return Ok(None);
    };
    let metadata: ExecutionMetadata = decode_row(row)?;
    let (transaction, nodes) =
        node_gates(transaction, &metadata.request_json, &asked.nodes).await?;
    commit(transaction).await?;
    Ok(Some(NodeGates { nodes }))
}
