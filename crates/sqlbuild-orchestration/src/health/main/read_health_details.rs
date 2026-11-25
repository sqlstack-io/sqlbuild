//! Health details for a person who may manage people, as the server settings are.

use uuid::Uuid;

use crate::authorization::main::decisions::begin_authorized::begin_authorized;
use crate::credentials::models::{Action, AuditContext, DefaultRole, PermissionScope};
use crate::health::_helpers::details::health_details;
use crate::health::models::{HealthDetails, ServerHealth};
use crate::store::errors::ControlError;
use crate::store::main::commit::commit;
use crate::store::models::ControlStore;

/// `health`'s details, once the person is authorized to manage people.
pub async fn read_health_details(
    (store, health): (&ControlStore, &ServerHealth),
    credentials: (&AuditContext, Uuid),
) -> Result<HealthDetails, ControlError> {
    let (transaction, _) = begin_authorized(
        store,
        credentials,
        &PermissionScope::action(Action::UserManage, None),
        DefaultRole::None,
    )
    .await?;
    commit(transaction).await?;
    Ok(health_details(health))
}
