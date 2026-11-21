//! One dim line of the detailed log, shown with `l`.

use crate::screen_text::_helpers::rows::{break_cells, paint, row_width, span};
use crate::screen_text::models::{ScreenFacts, Tone};

/// `line` dimmed and broken to the terminal width, never cut; control characters are dropped.
#[must_use]
pub fn render_log_line(line: &str, facts: &ScreenFacts) -> Vec<String> {
    break_cells(line, row_width(facts))
        .into_iter()
        .map(|row| paint(&[span(row, Tone::Muted)], facts))
        .collect()
}
