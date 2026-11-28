//! The engines a run's execution header names.

use crate::run_on::constants::DUCKDB_ENGINE;
use crate::run_on::models::{RunOn, RunOnDecision};

/// `DuckDB` when items run locally, `DuckDB and <adapter>` when some also run on the warehouse; `None` when none run locally.
#[must_use]
pub fn execution_engines(
    decisions: &[RunOnDecision],
    mode: RunOn,
    adapter: &str,
) -> Option<String> {
    let local: bool = decisions
        .iter()
        .any(|decision| decision.local() && (mode != RunOn::Local || decision.refusals.is_empty()));
    let warehouse: bool =
        mode != RunOn::Local && decisions.iter().any(|decision| !decision.local());
    match (local, warehouse) {
        (false, _) => None,
        (true, false) => Some(DUCKDB_ENGINE.to_owned()),
        (true, true) => Some(format!("{DUCKDB_ENGINE} and {adapter}")),
    }
}
