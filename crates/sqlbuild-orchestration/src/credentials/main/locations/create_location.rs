//! Register a location and its security audit atomically.

use uuid::Uuid;

use crate::credentials::_helpers::locations::create_location_in;
use crate::credentials::models::AuditContext;
use crate::store::errors::ControlError;
use crate::store::main::write::write;
use crate::store::models::ControlStore;

/// Names are labels, never credentials or stable identifiers.
pub async fn create_location(
    store: &ControlStore,
    name: &str,
    context: &AuditContext,
) -> Result<Uuid, ControlError> {
    write(store, |transaction| {
        create_location_in(transaction, name, context)
    })
    .await
}
