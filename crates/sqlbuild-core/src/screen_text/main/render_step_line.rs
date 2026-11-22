//! One step line: glyph, label, time, then what the step found or waits on.

use crate::screen_text::_helpers::phase_tones::phase_tones;
use crate::screen_text::_helpers::rows::{
    elapsed_text, fit_row, pad, paint, right, row_width, span, wrap_width, wrap_words,
};
use crate::screen_text::constants::{STEP_GAP, STEP_LABEL_WIDTH, STEP_TIME_WIDTH};
use crate::screen_text::models::{ScreenFacts, Segment, StepLine};
use crate::text::main::cell_len::cell_len;

/// `✓ Compiled project        2.4s   0 errors`: one row cut to the terminal while it is live, or every row of its wrapped detail once it is printed for good.
#[must_use]
pub fn render_step_line(line: &StepLine, facts: &ScreenFacts, lasting: bool) -> Vec<String> {
    let tones = phase_tones(line.state);
    let time: String = line.elapsed.map(elapsed_text).unwrap_or_default();
    let mut label: String = pad(&line.label, STEP_LABEL_WIDTH);
    if cell_len(&line.label) >= STEP_LABEL_WIDTH && !time.is_empty() {
        label.push(' ');
    }
    let lead: Vec<Segment> = vec![
        span(tones.glyph, tones.glyph_tone),
        span(" ", tones.label),
        span(label, tones.label),
        span(right(&time, STEP_TIME_WIDTH), tones.time),
    ];
    let column: usize = lead
        .iter()
        .map(|segment| cell_len(&segment.text))
        .sum::<usize>()
        + STEP_GAP.len();
    if !lasting {
        let mut segments: Vec<Segment> = lead;
        segments.push(span(STEP_GAP, tones.detail));
        segments.push(span(line.detail.as_str(), tones.detail));
        return vec![fit_row(segments, row_width(facts), facts)];
    }
    let detail: Vec<String> = if line.detail.is_empty() {
        vec![String::new()]
    } else {
        wrap_words(&line.detail, wrap_width(facts, column))
    };
    let mut rows: Vec<String> = Vec::new();
    for (index, text) in detail.into_iter().enumerate() {
        let mut segments: Vec<Segment> = if index == 0 {
            let mut first: Vec<Segment> = lead.clone();
            first.push(span(STEP_GAP, tones.detail));
            first
        } else {
            vec![span(" ".repeat(column), tones.detail)]
        };
        segments.push(span(text, tones.detail));
        rows.push(paint(&segments, facts));
    }
    rows
}
