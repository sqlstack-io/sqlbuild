//! Lifecycle changes share one transaction owner across trusted callers and person operations.

use crate::runs::_helpers::lifecycle::change_run::change_run_in;
use crate::runs::models::{RunAuthority, RunChangeRequest};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};

pub async fn change_run(
    store: &ControlStore,
    authority: RunAuthority<'_>,
    request: &RunChangeRequest,
) -> Result<bool, ControlError> {
    let (transaction, applied) = change_run_in(
        begin(store, TransactionMode::Write).await?,
        authority,
        request,
    )
    .await?;
    commit(transaction).await?;
    Ok(applied)
}
