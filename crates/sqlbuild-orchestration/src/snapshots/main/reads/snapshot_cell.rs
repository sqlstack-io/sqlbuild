//! A snapshot's cell in a cache of values made once per snapshot.

use std::sync::{Arc, Mutex, PoisonError};

use tokio::sync::OnceCell;
use uuid::Uuid;

use crate::snapshots::models::{SnapshotCell, SnapshotCells};

/// The snapshot's cell, marked most recently used; adding one evicts its scope's least recently used beyond `per_scope`.
pub(crate) fn snapshot_cell<S: PartialEq + Clone, T>(
    cells: &Mutex<SnapshotCells<S, T>>,
    (scope, snapshot_id): (S, Uuid),
    per_scope: usize,
) -> Arc<OnceCell<T>> {
    let mut state = cells.lock().unwrap_or_else(PoisonError::into_inner);
    state.clock += 1;
    let now = state.clock;
    if let Some(slot) = state
        .slots
        .iter_mut()
        .find(|slot| slot.snapshot_id == snapshot_id)
    {
        slot.used = now;
        return Arc::clone(&slot.cell);
    }
    let cell = Arc::new(OnceCell::new());
    state.slots.push(SnapshotCell {
        scope: scope.clone(),
        snapshot_id,
        used: now,
        cell: Arc::clone(&cell),
    });
    while state
        .slots
        .iter()
        .filter(|slot| slot.scope == scope)
        .count()
        > per_scope
    {
        let Some(oldest) = state
            .slots
            .iter()
            .enumerate()
            .filter(|(_, slot)| slot.scope == scope)
            .min_by_key(|(_, slot)| slot.used)
            .map(|(index, _)| index)
        else {
            break;
        };
        state.slots.remove(oldest);
    }
    cell
}
