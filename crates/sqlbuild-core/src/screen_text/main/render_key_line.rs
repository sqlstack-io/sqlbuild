//! The key line under the live steps.

use crate::screen_text::_helpers::rows::{fit_row, row_width, span};
use crate::screen_text::constants::SEPARATOR;
use crate::screen_text::models::{Hotkey, ScreenFacts, Segment, Tone};

/// `keys  o open UI · l full log · q quit`, cut to the terminal.
#[must_use]
pub fn render_key_line(keys: &[Hotkey], facts: &ScreenFacts) -> String {
    let mut segments: Vec<Segment> = vec![span("keys  ", Tone::Muted)];
    for (index, key) in keys.iter().enumerate() {
        if index > 0 {
            segments.push(span(SEPARATOR, Tone::Muted));
        }
        segments.push(span(key.key.to_string(), Tone::Strong));
        segments.push(span(format!(" {}", key.label), Tone::Muted));
    }
    fit_row(segments, row_width(facts), facts)
}
