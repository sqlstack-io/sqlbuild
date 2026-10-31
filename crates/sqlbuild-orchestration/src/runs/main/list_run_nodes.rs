//! Node outcomes are scoped to one frozen run, independently of current code snapshots.

use uuid::Uuid;

use crate::runs::models::RunNodeRecord;
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::decode_row::decode_row;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter, TransactionMode};

/// The API authorizes visibility of the run before reading its nodes.
pub async fn list_run_nodes(
    store: &ControlStore,
    run_id: Uuid,
) -> Result<Vec<RunNodeRecord>, ControlError> {
    let transaction = begin(store, TransactionMode::Read).await?;
    let (transaction, rows) = query(transaction,
        "SELECT node_key, attempt, status, started_at, ended_at, rows_affected, error_code, skipped_because_of \
         FROM sqb_run_nodes WHERE run_id=$1 ORDER BY node_key, attempt",
        vec![Parameter::Id(run_id)]).await?;
    let nodes = rows
        .into_iter()
        .map(decode_row)
        .collect::<Result<Vec<_>, _>>()?;
    commit(transaction).await?;
    Ok(nodes)
}
