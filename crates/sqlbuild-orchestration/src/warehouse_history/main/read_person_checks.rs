//! Live view permission over the whole target and the newest result of every check.

use uuid::Uuid;

use crate::credentials::models::{AuditContext, DefaultRole};
use crate::snapshots::main::reads::listing_read::listing_read;
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;
use crate::warehouse_history::_helpers::checks::latest_results;
use crate::warehouse_history::models::CheckResult;

pub async fn read_person_checks(
    store: &ControlStore,
    credentials: (&AuditContext, Uuid),
    scope: (Uuid, &str),
    default_role: DefaultRole,
) -> Result<Vec<CheckResult>, ControlError> {
    let rows = listing_read(store, (credentials, default_role), scope, |transaction| {
        latest_results(transaction, scope)
    })
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| CheckResult::new(row.event_id, row.record, row.ignored))
        .collect())
}
