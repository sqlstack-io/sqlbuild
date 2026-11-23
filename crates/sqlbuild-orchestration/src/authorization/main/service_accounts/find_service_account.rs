//! The CLI names service accounts; the server's routes number them.

use uuid::Uuid;

use crate::authorization::_helpers::service_accounts::service_account_named;
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;

/// The principal of the service account named `name`.
pub async fn find_service_account(store: &ControlStore, name: &str) -> Result<Uuid, ControlError> {
    service_account_named(store, name).await
}
