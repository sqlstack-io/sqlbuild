//! Live view permission over the whole target and the newest result of every check.

use uuid::Uuid;

use crate::authorization::main::authorize_listing::authorize_listing;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};
use crate::warehouse_history::_helpers::checks::latest_results;
use crate::warehouse_history::models::CheckResult;

pub async fn read_person_checks(
    store: &ControlStore,
    credentials: (&AuditContext, Uuid),
    (location_id, target): (Uuid, &str),
    default_role: DefaultRole,
) -> Result<Vec<CheckResult>, ControlError> {
    let (transaction, _) = authorize_listing(
        begin(store, TransactionMode::Write).await?,
        credentials,
        (location_id, target),
        default_role,
    )
    .await?;
    let (transaction, rows) = latest_results(transaction, (location_id, target)).await?;
    commit(transaction).await?;
    Ok(rows
        .into_iter()
        .map(|row| CheckResult::new(row.event_id, row.record))
        .collect())
}
