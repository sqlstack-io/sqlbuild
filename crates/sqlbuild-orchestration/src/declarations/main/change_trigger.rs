//! Commit an operator fact with its projection and security audit.

use crate::credentials::models::AuditContext;
use crate::declarations::_helpers::operator_change::change_trigger_in;
use crate::declarations::models::TriggerChangeRequest;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};

/// Authorization belongs to the API boundary; `actor` is authenticated, not client input.
pub async fn change_trigger(
    store: &ControlStore,
    context: &AuditContext,
    request: &TriggerChangeRequest,
) -> Result<bool, ControlError> {
    let (transaction, applied) = change_trigger_in(
        begin(store, TransactionMode::Write).await?,
        context,
        request,
    )
    .await?;
    commit(transaction).await?;
    Ok(applied)
}
