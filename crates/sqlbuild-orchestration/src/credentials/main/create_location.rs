//! Register a location and its security audit atomically.

use uuid::Uuid;

use crate::credentials::_helpers::locations::create_location_in;
use crate::credentials::models::AuditContext;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};

/// Names are labels, never credentials or stable identifiers.
pub async fn create_location(
    store: &ControlStore,
    name: &str,
    context: &AuditContext,
) -> Result<Uuid, ControlError> {
    let (transaction, id) =
        create_location_in(begin(store, TransactionMode::Write).await?, name, context).await?;
    commit(transaction).await?;
    Ok(id)
}
