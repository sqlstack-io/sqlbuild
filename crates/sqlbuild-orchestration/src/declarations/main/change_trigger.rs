//! Commit an operator fact with its projection and security audit.

use crate::credentials::models::AuditContext;
use crate::declarations::_helpers::operator_change::change_trigger_in;
use crate::declarations::models::TriggerChangeRequest;
use crate::store::errors::ControlError;
use crate::store::main::write::write;
use crate::store::models::ControlStore;

/// Authorization belongs to the API boundary; `actor` is authenticated, not client input.
pub async fn change_trigger(
    store: &ControlStore,
    context: &AuditContext,
    request: &TriggerChangeRequest,
) -> Result<bool, ControlError> {
    write(store, |transaction| {
        change_trigger_in(transaction, context, request)
    })
    .await
}
