//! A listing needs view of every node in the current snapshot; its refusal names no node and reads the same for unknown locations.

use std::collections::BTreeSet;

use uuid::Uuid;

use crate::authorization::constants::LISTING_DENIED;
use crate::authorization::main::authorize_person::authorize_person;
use crate::credentials::models::{Action, AuditContext, DefaultRole, PermissionScope};
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn authorize_listing(
    transaction: ControlTransaction,
    credentials: (&AuditContext, Uuid),
    (location_id, target): (Uuid, &str),
    default_role: DefaultRole,
) -> Result<(ControlTransaction, AuditContext), ControlError> {
    let (transaction, rows) = query(transaction,
        "SELECT n.node_key FROM sqb_snapshot_nodes n JOIN sqb_current_snapshots c ON c.snapshot_id=n.snapshot_id \
         WHERE c.location_id=$1 AND c.target=$2",
        vec![Parameter::Id(location_id), Parameter::Text(target.into())]).await?;
    let mut nodes: BTreeSet<String> = BTreeSet::new();
    for row in &rows {
        nodes.insert(row["node_key"].as_str().unwrap_or_default().into());
    }
    let scope = PermissionScope {
        action: Action::View,
        location_id: Some(location_id),
        target: Some(target.into()),
        nodes,
    };
    authorize_person(transaction, credentials, &scope, default_role)
        .await
        .map_err(|error| match error {
            ControlError::Denied { decision_id, .. } => ControlError::Denied {
                reason: LISTING_DENIED.into(),
                decision_id,
            },
            error => error,
        })
}
