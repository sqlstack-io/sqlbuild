//! One dim line of the detailed log shown under the `sqb dev` events.

use crate::screen_text::_helpers::rows::{render_row, span};
use crate::screen_text::models::{ScreenFacts, Tone};

/// `line` dimmed and cut to the screen width; control characters are dropped.
#[must_use]
pub fn render_log_line(line: &str, facts: &ScreenFacts) -> String {
    let text: String = line
        .chars()
        .filter(|character| !character.is_control())
        .collect();
    render_row(vec![span(text, Tone::Muted)], facts)
}
