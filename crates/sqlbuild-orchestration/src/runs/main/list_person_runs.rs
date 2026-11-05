//! Metadata reads hold live permission checks and results in the same transaction.

use uuid::Uuid;

use crate::authorization::main::begin_authorized::begin_authorized;
use crate::credentials::models::{Action, AuditContext, DefaultRole, PermissionScope};
use crate::runs::_helpers::lifecycle::reads::list_runs_in;
use crate::runs::models::RunRecord;
use crate::store::errors::ControlError;
use crate::store::main::commit::commit;
use crate::store::models::ControlStore;

pub async fn list_person_runs(
    store: &ControlStore,
    (context, credentials): (&AuditContext, Uuid),
    (location_id, target, limit): (Uuid, &str, u32),
    default_role: DefaultRole,
) -> Result<Vec<RunRecord>, ControlError> {
    let (transaction, _) = begin_authorized(
        store,
        (context, credentials),
        &PermissionScope::action(Action::View, Some((location_id, target))),
        default_role,
    )
    .await?;
    let (transaction, runs) = list_runs_in(transaction, (location_id, target), limit).await?;
    commit(transaction).await?;
    Ok(runs)
}
