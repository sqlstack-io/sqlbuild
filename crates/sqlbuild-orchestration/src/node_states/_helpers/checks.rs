//! A target's held check list: a read keeps its list only when no signal named the target since the read began.

use std::sync::PoisonError;

use crate::node_states::models::{HeldCheckList, TargetCache};

/// The signal generation now, and the list held at it.
pub(crate) fn held_checks(entry: &TargetCache) -> (u64, Option<HeldCheckList>) {
    let held = entry.checks.lock().unwrap_or_else(PoisonError::into_inner);
    (held.generation, held.list.clone())
}

/// Keep `list`, read when the generation was `generation`, unless a signal has named the target since.
pub(crate) fn keep_checks(entry: &TargetCache, generation: u64, list: HeldCheckList) {
    let mut held = entry.checks.lock().unwrap_or_else(PoisonError::into_inner);
    if held.generation == generation {
        held.list = Some(list);
    }
}

/// A signal named the target: drop its list, and any list a read in flight would keep.
pub(crate) fn drop_checks(entry: &TargetCache) {
    let mut held = entry.checks.lock().unwrap_or_else(PoisonError::into_inner);
    held.generation += 1;
    held.list = None;
}
