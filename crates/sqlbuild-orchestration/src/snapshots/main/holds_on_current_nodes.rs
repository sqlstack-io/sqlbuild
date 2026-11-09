//! Decide whether a person holds one action on every current node of a target, for checks that must not audit.

use std::collections::BTreeSet;

use uuid::Uuid;

use crate::authorization::main::decide_person_scopes::decide_person_scopes;
use crate::credentials::models::{Action, PermissionScope};
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

/// Whether the person holds `action` on every node of the target's current snapshot, decided without recording a denial.
pub(crate) async fn holds_on_current_nodes(
    transaction: ControlTransaction,
    principal_id: Uuid,
    (action, location_id, target): (Action, Uuid, &str),
) -> Result<(ControlTransaction, bool), ControlError> {
    let (transaction, current) = query(transaction,
        "SELECT n.node_key FROM sqb_snapshot_nodes n JOIN sqb_current_snapshots c ON c.snapshot_id=n.snapshot_id \
         WHERE c.location_id=$1 AND c.target=$2",
        vec![Parameter::Id(location_id), Parameter::Text(target.into())]).await?;
    let scope = PermissionScope {
        action,
        location_id: Some(location_id),
        target: Some(target.into()),
        nodes: current
            .iter()
            .filter_map(|row| row["node_key"].as_str().map(str::to_owned))
            .collect::<BTreeSet<String>>(),
    };
    let (transaction, refusals) =
        decide_person_scopes(transaction, principal_id, (target, &[scope])).await?;
    Ok((transaction, refusals.as_deref() == Some(&[None])))
}
