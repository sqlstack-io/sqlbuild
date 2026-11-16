//! One timestamped event line of the `sqb dev` screen.

use crate::screen_text::_helpers::rows::{column, elapsed_text, render_row, right, span};
use crate::screen_text::constants::{
    CHANGED_GLYPH, EVENT_CLOCK_WIDTH, EVENT_DETAIL_WIDTH, EVENT_TITLE_WIDTH, FAIL_GLYPH, OK_GLYPH,
    RESTARTED_GLYPH, STARTED_GLYPH, TIME_WIDTH, WARN_GLYPH,
};
use crate::screen_text::models::{EventLine, EventMark, ScreenFacts, Segment, Tone};

/// `09:41:03  ✓ title  detail  1.4s`, then each note indented under the title.
#[must_use]
pub fn render_event_line(event: &EventLine, facts: &ScreenFacts) -> Vec<String> {
    let (glyph, tone): (&str, Tone) = match event.mark {
        EventMark::Changed => (CHANGED_GLYPH, Tone::Change),
        EventMark::Succeeded => (OK_GLYPH, Tone::Success),
        EventMark::Failed => (FAIL_GLYPH, Tone::Error),
        EventMark::Restarted => (RESTARTED_GLYPH, Tone::Warning),
        EventMark::Started => (STARTED_GLYPH, Tone::Link),
        EventMark::Warning => (WARN_GLYPH, Tone::Warning),
    };
    let title_tone: Tone = if event.mark == EventMark::Failed {
        Tone::Error
    } else {
        Tone::Plain
    };
    let mut segments: Vec<Segment> = vec![
        span(format!("{}  ", event.clock), Tone::Muted),
        span(format!("{glyph} "), tone),
    ];
    if event.detail.is_empty() && event.elapsed.is_none() {
        segments.push(span(event.title.as_str(), title_tone));
    } else {
        segments.push(span(
            column(&event.title, EVENT_TITLE_WIDTH - 1),
            title_tone,
        ));
        segments.push(span(
            format!(" {}", column(&event.detail, EVENT_DETAIL_WIDTH - 1)),
            Tone::Muted,
        ));
        let time: String = event.elapsed.map(elapsed_text).unwrap_or_default();
        segments.push(span(
            format!(" {}", right(&time, TIME_WIDTH - 1)),
            Tone::Muted,
        ));
    }
    let mut rows: Vec<String> = vec![render_row(segments, facts)];
    for note in &event.notes {
        let mut note_segments: Vec<Segment> =
            vec![span(" ".repeat(EVENT_CLOCK_WIDTH + 2), Tone::Plain)];
        note_segments.extend(note.iter().cloned());
        rows.push(render_row(note_segments, facts));
    }
    rows
}
