//! The `sqb dev` header, printed once.

use crate::screen_text::_helpers::rows::{paint, row_width, span, wrap_words};
use crate::screen_text::constants::{PRODUCT, SEPARATOR};
use crate::screen_text::models::{DevHeader, ScreenFacts, Segment, Tone};
use crate::text::main::cell_len::cell_len;

/// `sqb dev  shop · target dev · sqb 0.256.0`, wrapped when the terminal is narrower.
#[must_use]
pub fn render_dev_header(header: &DevHeader, facts: &ScreenFacts) -> Vec<String> {
    let mut segments: Vec<Segment> = vec![
        span(PRODUCT, Tone::Strong),
        span("  ", Tone::Plain),
        span(header.project.as_str(), Tone::Plain),
    ];
    if !header.target.is_empty() {
        segments.push(span(
            format!("{SEPARATOR}target {}", header.target),
            Tone::Muted,
        ));
    }
    segments.push(span(
        format!("{SEPARATOR}sqb {}", header.version),
        Tone::Muted,
    ));
    let text: String = segments
        .iter()
        .map(|segment| segment.text.as_str())
        .collect();
    if cell_len(&text) <= row_width(facts) {
        return vec![paint(&segments, facts)];
    }
    wrap_words(&text, row_width(facts))
}
