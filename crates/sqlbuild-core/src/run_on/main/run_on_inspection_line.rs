//! The `--inspect` line that says where one test or scenario runs.

use crate::run_on::constants::RUNS_ON_LABEL;
use crate::run_on::models::RunOnDecision;

/// `  runs on: local`, or `  runs on: warehouse  (<reason>)`, with a newline.
#[must_use]
pub fn run_on_inspection_line(decision: &RunOnDecision) -> String {
    match decision.reason() {
        Some(reason) if !decision.local() => {
            format!("  {RUNS_ON_LABEL}: {}  ({reason})\n", decision.run_on)
        }
        _ => format!("  {RUNS_ON_LABEL}: {}\n", decision.run_on),
    }
}
