//! Decide whether a person holds one action on every listed node of a target, for checks that must not audit.

use std::collections::BTreeSet;

use uuid::Uuid;

use crate::authorization::main::decide_person_scopes::decide_person_scopes;
use crate::credentials::models::{Action, PermissionScope};
use crate::store::errors::ControlError;
use crate::store::models::ControlTransaction;

/// Whether the person holds `action` on every one of `nodes`, decided without recording a denial.
pub(crate) async fn holds_on_nodes(
    transaction: ControlTransaction,
    principal_id: Uuid,
    (action, location_id, target): (Action, Uuid, &str),
    nodes: BTreeSet<String>,
) -> Result<(ControlTransaction, bool), ControlError> {
    let scope = PermissionScope {
        action,
        location_id: Some(location_id),
        target: Some(target.into()),
        nodes,
    };
    let (transaction, refusals) =
        decide_person_scopes(transaction, principal_id, (target, &[scope])).await?;
    Ok((transaction, refusals.as_deref() == Some(&[None])))
}
