//! The hotkey bar at the bottom of the `sqb dev` screen.

use crate::screen_text::_helpers::rows::{render_row, span};
use crate::screen_text::constants::SEPARATOR;
use crate::screen_text::models::{Hotkey, ScreenFacts, Segment, Tone};

/// `o open UI · r recompile · q quit`, dim apart from the keys.
#[must_use]
pub fn render_hotkey_bar(keys: &[Hotkey], facts: &ScreenFacts) -> String {
    let mut segments: Vec<Segment> = Vec::new();
    for (index, hotkey) in keys.iter().enumerate() {
        if index > 0 {
            segments.push(span(SEPARATOR, Tone::Muted));
        }
        segments.push(span(hotkey.key.to_string(), Tone::Plain));
        segments.push(span(format!(" {}", hotkey.label), Tone::Muted));
    }
    render_row(segments, facts)
}
