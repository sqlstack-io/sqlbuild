//! Read the immutable execution contract only for the run's currently active claim.

use std::collections::BTreeMap;

use uuid::Uuid;

use crate::runs::_helpers::groups::selection::skipped_nodes;
use crate::runs::models::{ExecutionMetadata, RunClaim, RunExecution};
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::decode_row::decode_row;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter, TransactionMode};

/// A redeploy cannot change a claimed run's code or environment expectations.
pub async fn run_execution(
    store: &ControlStore,
    (run_id, claim_id): (Uuid, Uuid),
) -> Result<Option<RunExecution>, ControlError> {
    let transaction = begin(store, TransactionMode::Read).await?;
    let (transaction, rows) = query(
        transaction,
        "SELECT r.request_json, s.environment_hash, s.sqb_version FROM sqb_runs r \
         JOIN sqb_run_liveness l ON l.run_id=r.run_id \
         JOIN sqb_snapshots s ON s.snapshot_id=r.snapshot_id \
         WHERE r.run_id=$1 AND l.worker_instance=$2",
        vec![Parameter::Id(run_id), Parameter::Id(claim_id)],
    )
    .await?;
    let Some(row) = rows.into_iter().next() else {
        commit(transaction).await?;
        return Ok(None);
    };
    let metadata: ExecutionMetadata = decode_row(row)?;
    let (transaction, rows) = query(
        transaction,
        "SELECT node_key, node_hash FROM sqb_snapshot_nodes WHERE snapshot_id=$1",
        vec![Parameter::Id(metadata.request_json.snapshot_id)],
    )
    .await?;
    let node_hashes: BTreeMap<String, String> = rows
        .into_iter()
        .map(|row| {
            let fields: BTreeMap<String, String> = decode_row(row)?;
            Ok((fields["node_key"].clone(), fields["node_hash"].clone()))
        })
        .collect::<Result<_, ControlError>>()?;
    let (transaction, skipped_nodes) = skipped_nodes(transaction, &metadata.request_json).await?;
    commit(transaction).await?;
    Ok(Some(RunExecution {
        claim: RunClaim {
            claim_id,
            request: metadata.request_json,
        },
        environment_hash: metadata.environment_hash,
        sqb_version: metadata.sqb_version,
        node_hashes,
        skipped_nodes,
    }))
}
