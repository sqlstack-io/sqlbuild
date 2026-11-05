//! Live View authority and bounded historical estimates share one writer transaction.

use uuid::Uuid;

use crate::credentials::models::{Action, AuditContext, DefaultRole, PermissionScope};
use crate::store::errors::ControlError;
use crate::store::main::commit::commit;
use crate::store::models::ControlStore;
use crate::warehouse_metadata::_helpers::person_reads::begin_view;
use crate::warehouse_metadata::_helpers::statistics::read_statistics_page;
use crate::warehouse_metadata::models::{StatisticsPage, StatisticsQuery, StatisticsRetention};

pub async fn read_person_statistics(
    store: &ControlStore,
    (context, credentials): (&AuditContext, Uuid),
    (location_id, target, options): (Uuid, &str, &StatisticsQuery),
    (default_role, retention): (DefaultRole, StatisticsRetention),
) -> Result<StatisticsPage, ControlError> {
    let transaction = begin_view(
        store,
        (context, credentials),
        &PermissionScope {
            action: Action::View,
            location_id: Some(location_id),
            target: Some(target.into()),
            nodes: Default::default(),
        },
        default_role,
    )
    .await?;
    let (transaction, page) =
        read_statistics_page(transaction, (location_id, target), (options, retention)).await?;
    commit(transaction).await?;
    Ok(page)
}
