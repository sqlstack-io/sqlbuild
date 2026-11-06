//! Live view permission over the whole target and its recorded maintenance history.

use uuid::Uuid;

use crate::authorization::main::authorize_listing::authorize_listing;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};
use crate::warehouse_history::_helpers::maintenance::maintenance_history;
use crate::warehouse_history::models::{MaintenanceHistory, MaintenanceQuery};

pub async fn read_person_maintenance(
    store: &ControlStore,
    credentials: (&AuditContext, Uuid),
    (location_id, target, options): (Uuid, &str, &MaintenanceQuery),
    default_role: DefaultRole,
) -> Result<MaintenanceHistory, ControlError> {
    let (transaction, _) = authorize_listing(
        begin(store, TransactionMode::Write).await?,
        credentials,
        (location_id, target),
        default_role,
    )
    .await?;
    let (transaction, history) = maintenance_history(
        transaction,
        (location_id, target),
        (options.node.as_deref(), options.limit.unwrap_or(100)),
    )
    .await?;
    commit(transaction).await?;
    Ok(history)
}
