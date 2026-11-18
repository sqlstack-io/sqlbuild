//! Node outcomes are scoped to one frozen run, independently of current code snapshots.

use uuid::Uuid;

use crate::runs::_helpers::lifecycle::reads::run_nodes_in;
use crate::runs::models::RunNodeRecord;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};

/// The API authorizes visibility of the run before reading its nodes.
pub async fn list_run_nodes(
    store: &ControlStore,
    run_id: Uuid,
) -> Result<Vec<RunNodeRecord>, ControlError> {
    let transaction = begin(store, TransactionMode::Read).await?;
    let (transaction, nodes) = run_nodes_in(transaction, run_id).await?;
    commit(transaction).await?;
    Ok(nodes)
}
