//! Resolve a selection to every node it picks in the current publication, for a person who may view it.

use std::collections::BTreeSet;

use uuid::Uuid;

use crate::credentials::models::{AuditContext, DefaultRole};
use crate::runs::_helpers::lifecycle::publications::viewed_publication;
use crate::runs::models::{SelectionProblem, ViewedSelection};
use crate::runs::types::CommandPlanner;
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;

/// Every key `sqb build --select … --exclude …` picks; an empty selection is every node, even unpublished ones.
pub async fn select_viewed_nodes(
    store: &ControlStore,
    (planner, credentials): (&dyn CommandPlanner, (&AuditContext, Uuid)),
    (location_id, target): (Uuid, &str),
    ((select, exclude), default_role): ((&str, &str), DefaultRole),
) -> Result<ViewedSelection, ControlError> {
    let current = viewed_publication(
        store,
        (planner, credentials),
        (location_id, target),
        default_role,
    )
    .await?;
    let snapshot_id: Option<Uuid> = current.as_ref().map(|(snapshot, _)| snapshot.snapshot_id);
    if select.trim().is_empty() && exclude.trim().is_empty() {
        return Ok(ViewedSelection {
            snapshot_id,
            keys: None,
            error: None,
        });
    }
    let Some((_, publication)) = current else {
        return Ok(ViewedSelection {
            snapshot_id,
            keys: Some(BTreeSet::new()),
            error: Some(SelectionProblem {
                code: None,
                message: "this target has no publication yet".into(),
                help: None,
                location: None,
                suggestions: Vec::new(),
                refused: None,
            }),
        });
    };
    Ok(match publication.prepared.evaluate(select, exclude) {
        Ok(keys) => ViewedSelection {
            snapshot_id,
            keys: Some(keys.into_iter().collect()),
            error: None,
        },
        Err(problem) => ViewedSelection {
            snapshot_id,
            keys: Some(BTreeSet::new()),
            error: Some(*problem),
        },
    })
}
