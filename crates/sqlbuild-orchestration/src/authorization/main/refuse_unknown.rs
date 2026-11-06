//! An unknown object is refused like a forbidden one unless the person may act on the whole scope it was looked up in.

use std::collections::BTreeSet;

use uuid::Uuid;

use crate::authorization::main::authorize_person::authorize_person;
use crate::credentials::models::{Action, AuditContext, DefaultRole, PermissionScope};
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

/// `location` is where the lookup happened, or `None` when the object alone named it.
pub(crate) async fn refuse_unknown<T>(
    transaction: ControlTransaction,
    credentials: (&AuditContext, Uuid),
    (action, location): (Action, Option<(Uuid, &str)>),
    missing: ControlError,
) -> Result<T, ControlError> {
    let (transaction, nodes) = match location {
        Some((location_id, target)) => {
            let (transaction, rows) = query(transaction,
                "SELECT n.node_key FROM sqb_snapshot_nodes n JOIN sqb_current_snapshots c ON c.snapshot_id=n.snapshot_id \
                 WHERE c.location_id=$1 AND c.target=$2",
                vec![Parameter::Id(location_id), Parameter::Text(target.into())]).await?;
            let nodes: BTreeSet<String> = rows
                .iter()
                .filter_map(|row| row["node_key"].as_str().map(str::to_owned))
                .collect();
            (transaction, nodes)
        }
        None => (transaction, BTreeSet::new()),
    };
    let scope = PermissionScope {
        action,
        location_id: location.map(|(location_id, _)| location_id),
        target: location.map(|(_, target)| target.to_owned()),
        nodes,
    };
    authorize_person(transaction, credentials, &scope, DefaultRole::None).await?;
    Err(missing)
}
