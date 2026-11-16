//! A checklist of phases, one line each, with aligned labels and right-aligned times.

use crate::screen_text::_helpers::phase_tones::{PhaseTones, phase_tones};
use crate::screen_text::_helpers::rows::{column, elapsed_text, render_row, right, span};
use crate::screen_text::constants::{PHASE_LABEL_WIDTH, TIME_WIDTH};
use crate::screen_text::models::{PhaseLayout, PhaseLine, ScreenFacts, Segment, Tone};
use crate::text::main::cell_len::cell_len;

/// The glyph, label, detail and time columns that `PhaseLine` in `layout` puts on each row.
#[must_use]
pub fn render_phase_lines(
    lines: &[PhaseLine],
    layout: PhaseLayout,
    facts: &ScreenFacts,
) -> Vec<String> {
    let label_width: usize = lines
        .iter()
        .map(|line| cell_len(&line.label))
        .max()
        .unwrap_or(0)
        .max(PHASE_LABEL_WIDTH);
    let fixed: usize = 2 + label_width + 1 + 1 + TIME_WIDTH;
    let detail_width: usize = lines
        .iter()
        .map(|line| cell_len(&line.detail))
        .max()
        .unwrap_or(0)
        .min(facts.columns.saturating_sub(facts.indent + fixed));
    lines
        .iter()
        .map(|line| {
            let tones: PhaseTones = phase_tones(line.state);
            let time: String = line.elapsed.map(elapsed_text).unwrap_or_default();
            let mut segments: Vec<Segment> = vec![
                span(tones.glyph, tones.glyph_tone),
                span(" ", Tone::Plain),
                span(column(&line.label, label_width), tones.label),
            ];
            match layout {
                PhaseLayout::TimeLast => {
                    segments.push(span(" ", Tone::Plain));
                    segments.push(span(column(&line.detail, detail_width), tones.detail));
                    segments.push(span(format!(" {}", right(&time, TIME_WIDTH)), tones.time));
                }
                PhaseLayout::TimeFirst => {
                    segments.push(span(format!(" {}", right(&time, TIME_WIDTH)), tones.time));
                    segments.push(span(format!("   {}", line.detail), tones.detail));
                }
            }
            render_row(segments, facts)
        })
        .collect()
}
