//! Live View authority and bounded historical estimates share one writer transaction.

use uuid::Uuid;

use crate::authorization::main::begin_authorized::begin_authorized;
use crate::credentials::models::{Action, AuditContext, DefaultRole, PermissionScope};
use crate::store::errors::ControlError;
use crate::store::main::commit::commit;
use crate::store::models::ControlStore;
use crate::warehouse_metadata::_helpers::statistics::read_statistics_page;
use crate::warehouse_metadata::models::{StatisticsPage, StatisticsQuery, StatisticsRetention};

pub async fn read_person_statistics(
    store: &ControlStore,
    (context, credentials): (&AuditContext, Uuid),
    (location_id, target, options): (Uuid, &str, &StatisticsQuery),
    (default_role, retention): (DefaultRole, StatisticsRetention),
) -> Result<StatisticsPage, ControlError> {
    let (transaction, _) = begin_authorized(
        store,
        (context, credentials),
        &PermissionScope::action(Action::View, Some((location_id, target))),
        default_role,
    )
    .await?;
    let (transaction, page) =
        read_statistics_page(transaction, (location_id, target), (options, retention)).await?;
    commit(transaction).await?;
    Ok(page)
}
