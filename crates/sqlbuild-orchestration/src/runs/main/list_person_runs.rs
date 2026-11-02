//! Metadata reads hold live permission checks and results in the same transaction.

use uuid::Uuid;

use crate::authorization::main::authorize_session::authorize_session;
use crate::credentials::models::{Action, AuditContext, PermissionScope};
use crate::runs::_helpers::lifecycle::reads::list_runs_in;
use crate::runs::models::RunRecord;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};

pub async fn list_person_runs(
    store: &ControlStore,
    (context, credentials): (&AuditContext, (&str, &str)),
    (location_id, target, limit): (Uuid, &str, u32),
) -> Result<Vec<RunRecord>, ControlError> {
    let (transaction, _) = authorize_session(
        begin(store, TransactionMode::Write).await?,
        (context, credentials),
        &PermissionScope {
            action: Action::View,
            location_id: Some(location_id),
            target: Some(target.into()),
            nodes: Default::default(),
        },
    )
    .await?;
    let (transaction, runs) = list_runs_in(transaction, (location_id, target), limit).await?;
    commit(transaction).await?;
    Ok(runs)
}
