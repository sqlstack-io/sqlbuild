//! The live footer at the bottom of a terminal while tests or scenarios run.

use crate::cli_text::main::format_summary_footer::format_summary_footer;
use crate::cli_text::models::CliStyle;
use crate::run_output::_helpers::counts::summary_counts;
use crate::run_output::_helpers::line::Line;
use crate::run_output::constants::{FOOTER_MARGIN, LAST_GLYPH, MIDDLE_DOT};
use crate::run_output::main::cut_middle::cut_middle;
use crate::run_output::models::ProgressFooter;

/// Counts, then what is running and held back, then the oldest running item; each line fits `columns`.
#[must_use]
pub fn render_progress_footer(
    footer: &ProgressFooter,
    frame: &str,
    columns: usize,
    style: CliStyle,
) -> Vec<String> {
    let width: usize = columns.saturating_sub(FOOTER_MARGIN).max(1);
    let counts: String = format_summary_footer(
        style,
        &summary_counts(&footer.counts, false),
        Some(&format!("{:.1}s", footer.elapsed)),
    );
    let mut first: Line = Line::default();
    first
        .push(frame, |text| style.success(text))
        .text(" ")
        .push(&footer.verb, |text| style.section(text))
        .text("  ")
        .push(
            &format!("{}/{}", footer.counts.done(), footer.counts.total),
            |text| style.value(text),
        )
        .text("   ")
        .styled_piece(&counts);
    let mut lines: Vec<String> = vec![first.fit(width, style)];
    if footer.running == 0 && footer.held == 0 {
        return lines;
    }
    let mut state: String = format!("{} running", footer.running);
    if footer.held > 0 {
        state = format!("{state}, {} finished waiting to print", footer.held);
    }
    let mut second: Line = Line::default();
    second.text("  ").push(&state, |text| style.muted(text));
    if let Some((_, seconds)) = &footer.oldest {
        second
            .push(&format!(" {MIDDLE_DOT} oldest "), |text| style.muted(text))
            .push(&format!("{seconds:.1}s"), |text| style.warning(text));
    }
    lines.push(second.fit(width, style));
    if let Some((name, _)) = &footer.oldest {
        let prefix: String = format!("  {LAST_GLYPH} ");
        let room: usize = width.saturating_sub(prefix.chars().count()).max(1);
        lines.push(format!(
            "  {} {}",
            style.muted(LAST_GLYPH),
            cut_middle(name, room)
        ));
    }
    lines
}
