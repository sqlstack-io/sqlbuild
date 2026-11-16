//! Work on a location's published snapshot skips the rules gate: the location ran them at load.

use std::cell::Cell;

thread_local! {
    /// Whether the running work plans a snapshot whose rules passed when the location loaded it.
    static RULES_RAN_AT_LOAD: Cell<bool> = const { Cell::new(false) };
}

/// Marks plans and runs on a published snapshot for one scope, restored when the scope ends.
pub(crate) struct RulesAtLoadScope {
    previous: bool,
}

impl RulesAtLoadScope {
    pub(crate) fn enter() -> Self {
        Self {
            previous: RULES_RAN_AT_LOAD.replace(true),
        }
    }
}

impl Drop for RulesAtLoadScope {
    fn drop(&mut self) {
        RULES_RAN_AT_LOAD.set(self.previous);
    }
}

/// Whether the running work plans a published snapshot, whose rules passed at load.
pub(crate) fn rules_ran_at_load() -> bool {
    RULES_RAN_AT_LOAD.get()
}
