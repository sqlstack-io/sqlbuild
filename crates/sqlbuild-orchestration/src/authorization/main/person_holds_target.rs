//! Whether a person holds an action on every node of a target, unaudited: change notices recheck view before every notice, and reads trim detail with it.

use uuid::Uuid;

use crate::authorization::_helpers::person_grants::{
    GrantDecision, decide_grant, person_role_rows,
};
use crate::credentials::models::{Action, DefaultRole, PermissionScope};
use crate::snapshots::main::read_snapshot_keys::read_snapshot_keys;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

/// Whether an active person holds `action` on every current node of the target; with view, as listings require.
pub(crate) async fn person_holds_target(
    store: &ControlStore,
    principal_id: Uuid,
    (action, location_id, target): (Action, Uuid, &str),
    default_role: DefaultRole,
) -> Result<bool, ControlError> {
    let transaction = begin(store, TransactionMode::Read).await?;
    let (transaction, current) = query(
        transaction,
        "SELECT c.snapshot_id FROM sqb_current_snapshots c JOIN sqb_locations l \
         ON l.location_id=c.location_id WHERE l.archived_at IS NULL AND c.location_id=$1 AND c.target=$2",
        vec![Parameter::Id(location_id), Parameter::Text(target.into())],
    )
    .await?;
    let Some(snapshot_id) = current.first() else {
        commit(transaction).await?;
        return Ok(false);
    };
    let (transaction, nodes) = read_snapshot_keys(
        transaction,
        serde_json::from_value(snapshot_id["snapshot_id"].clone())
            .map_err(|error| ControlError::Invalid(error.to_string()))?,
    )
    .await?;
    let (transaction, rows) =
        person_role_rows(transaction, principal_id, Some(target.into())).await?;
    commit(transaction).await?;
    let Some(rows) = rows else {
        return Ok(false);
    };
    let scope = PermissionScope {
        action,
        location_id: Some(location_id),
        target: Some(target.into()),
        nodes,
    };
    Ok(matches!(
        decide_grant(&rows, &scope, default_role),
        GrantDecision::Granted(_)
    ))
}
