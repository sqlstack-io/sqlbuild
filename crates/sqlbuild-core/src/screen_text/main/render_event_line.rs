//! One timestamped event of the `sqb dev` screen, wrapped in full.

use crate::screen_text::_helpers::rows::{
    elapsed_text, paint, span, wrap_width, wrap_words,
};
use crate::screen_text::constants::{
    CHANGED_GLYPH, EVENT_TEXT_COLUMN, FAIL_GLYPH, OK_GLYPH, RESTARTED_GLYPH, STARTED_GLYPH,
    WARN_GLYPH,
};
use crate::screen_text::models::{EventLine, EventMark, ScreenFacts, Tone};

/// `14:20:31 ▸ run r_8f3a21c0  started manually`, its text and notes wrapped under one another.
#[must_use]
pub fn render_event_line(event: &EventLine, facts: &ScreenFacts) -> Vec<String> {
    let (glyph, tone): (&str, Tone) = match event.mark {
        EventMark::Changed => (CHANGED_GLYPH, Tone::Change),
        EventMark::Succeeded => (OK_GLYPH, Tone::Success),
        EventMark::Failed => (FAIL_GLYPH, Tone::Error),
        EventMark::Restarted => (RESTARTED_GLYPH, Tone::Warning),
        EventMark::Started => (STARTED_GLYPH, Tone::Change),
        EventMark::Warning => (WARN_GLYPH, Tone::Warning),
    };
    let text: String = [
        event.title.clone(),
        event.detail.clone(),
        event.elapsed.map(elapsed_text).unwrap_or_default(),
    ]
    .into_iter()
    .filter(|part| !part.is_empty())
    .collect::<Vec<String>>()
    .join("  ");
    let width: usize = wrap_width(facts, EVENT_TEXT_COLUMN);
    let hang: String = " ".repeat(EVENT_TEXT_COLUMN);
    let mut rows: Vec<String> = Vec::new();
    for (index, line) in wrap_words(&text, width).into_iter().enumerate() {
        rows.push(if index == 0 {
            paint(
                &[
                    span(format!("{} ", event.clock), Tone::Muted),
                    span(glyph, tone),
                    span(" ", Tone::Plain),
                    span(line, Tone::Plain),
                ],
                facts,
            )
        } else {
            paint(&[span(hang.as_str(), Tone::Plain), span(line, Tone::Plain)], facts)
        });
    }
    for note in &event.notes {
        for line in wrap_words(note, width) {
            rows.push(paint(
                &[span(hang.as_str(), Tone::Plain), span(line, Tone::Muted)],
                facts,
            ));
        }
    }
    rows
}
