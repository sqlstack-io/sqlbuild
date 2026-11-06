//! Report what a person may do without auditing; only display uses this, never a request's authorization.

use std::collections::BTreeSet;

use uuid::Uuid;

use crate::authorization::_helpers::person_grants::{allowed, person_role_rows, snapshot_nodes};
use crate::authorization::models::{LocationAccess, PersonAccess};
use crate::credentials::models::{Action, DefaultRole};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

/// `None` for a disabled or unknown person; locations without view on any node are left out.
pub(crate) async fn read_person_access(
    store: &ControlStore,
    principal_id: Uuid,
    default_role: DefaultRole,
) -> Result<Option<PersonAccess>, ControlError> {
    let transaction = begin(store, TransactionMode::Read).await?;
    let (transaction, people) = query(
        transaction,
        "SELECT name FROM sqb_principals WHERE principal_id=$1 AND disabled_at IS NULL",
        vec![Parameter::Id(principal_id)],
    )
    .await?;
    let Some(name) = people
        .first()
        .map(|row| row["name"].as_str().unwrap_or_default().to_owned())
    else {
        commit(transaction).await?;
        return Ok(None);
    };
    let (transaction, rows) = person_role_rows(transaction, principal_id, None).await?;
    let actions = allowed(
        &rows.unwrap_or_default(),
        (None, None),
        &BTreeSet::new(),
        DefaultRole::None,
    );
    let (mut transaction, targets) = query(
        transaction,
        "SELECT l.location_id,l.name,c.target,c.snapshot_id FROM sqb_current_snapshots c \
         JOIN sqb_locations l ON l.location_id=c.location_id WHERE l.archived_at IS NULL \
         ORDER BY l.name,c.target",
        Vec::new(),
    )
    .await?;
    let mut locations: Vec<LocationAccess> = Vec::new();
    for current in &targets {
        let location_id: Uuid = serde_json::from_value(current["location_id"].clone())
            .map_err(|error| ControlError::Invalid(error.to_string()))?;
        let target = current["target"].as_str().unwrap_or_default().to_owned();
        let (next, nodes) = snapshot_nodes(
            transaction,
            serde_json::from_value(current["snapshot_id"].clone())
                .map_err(|error| ControlError::Invalid(error.to_string()))?,
        )
        .await?;
        let (next, rows) = person_role_rows(next, principal_id, Some(target.clone())).await?;
        transaction = next;
        let actions = allowed(
            &rows.unwrap_or_default(),
            (Some(location_id), Some(target.clone())),
            &nodes,
            default_role,
        );
        if actions.contains(&Action::View) {
            locations.push(LocationAccess {
                location_id,
                name: current["name"].as_str().unwrap_or_default().to_owned(),
                target,
                actions,
            });
        }
    }
    commit(transaction).await?;
    Ok(Some(PersonAccess {
        principal_id,
        name,
        actions,
        locations,
    }))
}
