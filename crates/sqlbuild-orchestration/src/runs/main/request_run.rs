//! Queue a frozen, already authorized request without recreating work on transport retries.

use crate::runs::_helpers::requests::publication::publish_request;
use crate::runs::models::RunRequest;
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::models::{ControlStore, TransactionMode};

/// The API must compute and authorize this selection; callers cannot supply their own write scope.
pub async fn request_run(store: &ControlStore, request: &RunRequest) -> Result<bool, ControlError> {
    let transaction = begin(store, TransactionMode::Write).await?;
    let (transaction, applied) = publish_request(transaction, request).await?;
    commit(transaction).await?;
    Ok(applied)
}
