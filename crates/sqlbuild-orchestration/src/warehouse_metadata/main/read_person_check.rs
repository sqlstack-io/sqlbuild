//! Live view permission over the whole target and one check's results, newest first.

use uuid::Uuid;

use crate::authorization::main::authorize_listing::authorize_listing;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};
use crate::warehouse_metadata::_helpers::checks::check_results;
use crate::warehouse_metadata::models::CheckResult;

pub async fn read_person_check(
    store: &ControlStore,
    credentials: (&AuditContext, Uuid),
    (location_id, target, binding_key): (Uuid, &str, &str),
    (limit, default_role): (u32, DefaultRole),
) -> Result<Vec<CheckResult>, ControlError> {
    let (transaction, _) = authorize_listing(
        begin(store, TransactionMode::Write).await?,
        credentials,
        (location_id, target),
        default_role,
    )
    .await?;
    let (transaction, rows) =
        check_results(transaction, (location_id, target, binding_key), limit).await?;
    commit(transaction).await?;
    Ok(rows
        .into_iter()
        .map(|row| CheckResult::new(row.event_id, row.record))
        .collect())
}
