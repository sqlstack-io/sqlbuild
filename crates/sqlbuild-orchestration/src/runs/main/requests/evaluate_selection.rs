//! Resolve a selection as `sqb build` would, against the current publication, for a person who may view it.

use uuid::Uuid;

use crate::credentials::models::{AuditContext, DefaultRole};
use crate::runs::_helpers::lifecycle::publications::viewed_publication;
use crate::runs::_helpers::requests::past_runs::read_selected_past_runs;
use crate::runs::constants::{SELECTION_PAGE_DEFAULT, SELECTION_PAGE_MAX};
use crate::runs::models::{SelectionEvaluation, SelectionProblem, SelectionQuery};
use crate::runs::types::CommandPlanner;
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;

/// One page of the keys `sqb build --select … --exclude …` selects, or why it refuses the selectors.
pub async fn evaluate_selection(
    store: &ControlStore,
    (planner, credentials): (&dyn CommandPlanner, (&AuditContext, Uuid)),
    (location_id, target, query): (Uuid, &str, &SelectionQuery),
    default_role: DefaultRole,
) -> Result<SelectionEvaluation, ControlError> {
    let current = viewed_publication(
        store,
        (planner, credentials),
        (location_id, target),
        default_role,
    )
    .await?;
    let Some((snapshot, publication)) = current else {
        return Ok(SelectionEvaluation {
            snapshot_id: None,
            count: 0,
            keys: Vec::new(),
            next_offset: None,
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
    let past = read_selected_past_runs(
        store,
        (location_id, target),
        (publication.prepared.as_ref(), &query.select, &query.exclude),
    )
    .await?;
    let keys = match past.and_then(|past| {
        publication
            .prepared
            .evaluate((&query.select, &query.exclude), &past)
    }) {
        Ok(keys) => keys,
        Err(problem) => {
            return Ok(SelectionEvaluation {
                snapshot_id: Some(snapshot.snapshot_id),
                count: 0,
                keys: Vec::new(),
                next_offset: None,
                error: Some(*problem),
            });
        }
    };
    let offset = query.offset.unwrap_or(0);
    let limit = query
        .limit
        .unwrap_or(SELECTION_PAGE_DEFAULT)
        .clamp(1, SELECTION_PAGE_MAX);
    let page: Vec<String> = keys
        .iter()
        .skip(offset as usize)
        .take(limit as usize)
        .cloned()
        .collect();
    let end = offset.saturating_add(limit);
    Ok(SelectionEvaluation {
        snapshot_id: Some(snapshot.snapshot_id),
        count: keys.len() as u64,
        next_offset: ((end as usize) < keys.len()).then_some(end),
        keys: page,
        error: None,
    })
}
