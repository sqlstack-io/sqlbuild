//! The heartbeat a run writes when its output is not a terminal, in place of the live footer.

use crate::cli_text::models::CliStyle;
use crate::run_output::constants::{NOT_RUN_LABEL, TIMEOUT_LABEL};
use crate::run_output::models::ProgressFooter;

/// Counts and what is running, then the oldest running item in full; nothing is cut, logs have no width.
#[must_use]
pub fn render_progress_heartbeat(footer: &ProgressFooter, style: CliStyle) -> Vec<String> {
    let counts = &footer.counts;
    let mut first: String = format!(
        "  ... {}/{} {}  PASS={}  FAIL={}",
        counts.done(),
        counts.total,
        footer.noun,
        counts.pass,
        counts.fail
    );
    if counts.timeout > 0 {
        first = format!("{first}  {TIMEOUT_LABEL}={}", counts.timeout);
    }
    if counts.not_run > 0 {
        first = format!("{first}  {NOT_RUN_LABEL}={}", counts.not_run);
    }
    first = format!("{first}  {} running", footer.running);
    if footer.held > 0 {
        first = format!("{first}, {} finished waiting to print", footer.held);
    }
    first = format!("{first}  ({:.1}s)", footer.elapsed);
    let mut lines: Vec<String> = vec![style.muted(&first)];
    if let Some((name, seconds)) = &footer.oldest {
        lines.push(style.muted(&format!("      oldest {seconds:.1}s  {name}")));
    }
    lines
}
