//! The last lines of a run: its detail lines, then the summary line, which is always last.

use crate::cli_text::main::format_completion_line::format_completion_line;
use crate::cli_text::main::format_summary_footer::format_summary_footer;
use crate::cli_text::models::{CliStyle, CompletionState};
use crate::run_output::_helpers::counts::summary_counts;
use crate::run_output::constants::{ERRORS_LABEL, SUCCESS_LABEL};
use crate::run_output::models::RunCounts;

/// A blank line, each dim detail line, then `✓ Completed successfully` or `✗ Completed with errors` and the counts.
#[must_use]
pub fn render_run_completion(
    counts: &RunCounts,
    details: &[String],
    elapsed: f64,
    style: CliStyle,
) -> String {
    let summary: String = format_summary_footer(
        style,
        &summary_counts(counts, true),
        Some(&format!("{elapsed:.2}s")),
    );
    let (state, label): (CompletionState, &str) = if counts.all_passed() {
        (CompletionState::Ok, SUCCESS_LABEL)
    } else {
        (CompletionState::Fail, ERRORS_LABEL)
    };
    let mut lines: Vec<String> = vec![String::new()];
    lines.extend(
        details
            .iter()
            .map(|detail| style.muted(&format!("  {detail}"))),
    );
    lines.push(format_completion_line(style, state, label, Some(&summary)));
    format!("{}\n", lines.join("\n"))
}
