//! The ready card's trigger summary.

use sqlbuild_orchestration::declarations::models::{TriggerRecord, TriggerStatus};

/// `41 paused`, `3 running · 38 paused` or `none`.
pub(crate) fn trigger_summary(triggers: &[TriggerRecord]) -> String {
    let paused: usize = triggers
        .iter()
        .filter(|trigger| trigger.status == TriggerStatus::Paused)
        .count();
    let running: usize = triggers.len() - paused;
    match (running, paused) {
        (0, 0) => "none".to_owned(),
        (0, paused) => format!("{paused} paused"),
        (running, 0) => format!("{running} running"),
        (running, paused) => format!("{running} running \u{b7} {paused} paused"),
    }
}
