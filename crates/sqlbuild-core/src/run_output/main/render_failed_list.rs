//! `Failed (N):`: one line per failed item, its full name and a short reason, before the summary.

use crate::cli_text::main::format_overflow_line::format_overflow_line;
use crate::cli_text::models::CliStyle;
use crate::run_output::constants::{FAIL_GLYPH, FAILED_LIST_CAP, VERBOSE_FLAG};

/// The list of `(name, reason)` pairs, stopping at twenty unless `verbose`; empty with none.
#[must_use]
pub fn render_failed_list(failed: &[(String, String)], verbose: bool, style: CliStyle) -> String {
    if failed.is_empty() {
        return String::new();
    }
    let shown: usize = if verbose {
        failed.len()
    } else {
        failed.len().min(FAILED_LIST_CAP)
    };
    let mut lines: Vec<String> = vec![
        String::new(),
        style.error_strong(&format!("Failed ({}):", failed.len())),
    ];
    for (name, reason) in &failed[..shown] {
        let reason: String = if reason.is_empty() {
            String::new()
        } else {
            format!("  {}", style.muted(reason))
        };
        lines.push(format!("  {} {name}{reason}", style.error(FAIL_GLYPH)));
    }
    let hidden: i64 = i64::try_from(failed.len() - shown).unwrap_or(i64::MAX);
    lines.extend(format_overflow_line(hidden, "  ", VERBOSE_FLAG));
    format!("{}\n", lines.join("\n"))
}
