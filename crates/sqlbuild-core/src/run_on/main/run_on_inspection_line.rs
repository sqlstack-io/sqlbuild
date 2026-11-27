//! The `--inspect` lines that say where one test or scenario runs, and what it relied on.

use crate::run_on::constants::RUNS_ON_LABEL;
use crate::run_on::main::relied_on_text::relied_on_text;
use crate::run_on::models::RunOnDecision;

/// `  runs on: local`, or `  runs on: warehouse  (<reason>)`, with a newline, then a local item's `  relied on: ...` line.
#[must_use]
pub fn run_on_inspection_line(decision: &RunOnDecision) -> String {
    match (decision.reason(), relied_on_text(&decision.relied_on)) {
        (Some(reason), _) if !decision.local() => {
            format!("  {RUNS_ON_LABEL}: {}  ({reason})\n", decision.run_on)
        }
        (_, Some(relied)) => format!("  {RUNS_ON_LABEL}: {}\n  {relied}\n", decision.run_on),
        _ => format!("  {RUNS_ON_LABEL}: {}\n", decision.run_on),
    }
}
