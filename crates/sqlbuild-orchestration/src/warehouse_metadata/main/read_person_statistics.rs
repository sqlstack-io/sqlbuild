//! Live View authority and bounded historical estimates share one writer transaction.

use uuid::Uuid;

use crate::authorization::main::authorize_listing::authorize_listing;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};
use crate::warehouse_metadata::_helpers::statistics::read_statistics_page;
use crate::warehouse_metadata::models::{StatisticsPage, StatisticsQuery, StatisticsRetention};

pub async fn read_person_statistics(
    store: &ControlStore,
    (context, credentials): (&AuditContext, (&str, &str)),
    (location_id, target, options): (Uuid, &str, &StatisticsQuery),
    (default_role, retention): (DefaultRole, StatisticsRetention),
) -> Result<StatisticsPage, ControlError> {
    let (transaction, _) = authorize_listing(
        begin(store, TransactionMode::Write).await?,
        (context, credentials),
        (location_id, target),
        default_role,
    )
    .await?;
    let (transaction, page) =
        read_statistics_page(transaction, (location_id, target), (options, retention)).await?;
    commit(transaction).await?;
    Ok(page)
}
