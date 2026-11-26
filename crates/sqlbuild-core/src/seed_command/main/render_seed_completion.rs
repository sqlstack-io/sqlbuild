//! The seed completion line.

use crate::cli_text::main::format_completion_line::format_completion_line;
use crate::cli_text::main::format_summary_footer::format_summary_footer;
use crate::cli_text::models::{CliStyle, CompletionState, SummaryCount};

/// A blank line and the completion state with PASS, WARN, FAIL, SKIP and TOTAL counts.
#[must_use]
pub fn render_seed_completion(
    (passed, failed, total): (usize, usize, usize),
    elapsed_seconds: f64,
    style: CliStyle,
) -> String {
    let summary: String = format_summary_footer(
        style,
        &[
            SummaryCount::of("PASS", passed),
            SummaryCount::of("WARN", 0),
            SummaryCount::of("FAIL", failed),
            SummaryCount::of("SKIP", 0),
            SummaryCount::of("TOTAL", total),
        ],
        Some(&format!("{elapsed_seconds:.2}s")),
    );
    let (state, label) = if failed == 0 {
        (CompletionState::Ok, "Completed successfully")
    } else {
        (CompletionState::Fail, "Completed with errors")
    };
    format!(
        "\n{}\n",
        format_completion_line(style, state, label, Some(&summary))
    )
}
