//! Installation API reads never mix locations or targets.

use uuid::Uuid;

use crate::runs::_helpers::lifecycle::reads::list_runs_in;
use crate::runs::models::RunRecord;
use crate::runs::types::RunsListing;
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::models::{ControlStore, TransactionMode};

/// Return recent projected runs; per-run events remain in Recall.
pub async fn list_runs(
    store: &ControlStore,
    (location_id, target): (Uuid, &str),
    limit: u32,
) -> Result<Vec<RunRecord>, ControlError> {
    let transaction = begin(store, TransactionMode::Read).await?;
    let (transaction, runs) = list_runs_in(
        transaction,
        (location_id, target),
        RunsListing::Newest(limit, None),
    )
    .await?;
    commit(transaction).await?;
    Ok(runs)
}
