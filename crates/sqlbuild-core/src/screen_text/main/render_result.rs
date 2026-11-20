//! The line a `sqb dev` session ends with, and what to run next.

use crate::screen_text::_helpers::rows::{
    elapsed_text, pad, paint, row_width, span, wrap_width, wrap_words,
};
use crate::screen_text::constants::{FAIL_GLYPH, NEXT_COMMAND_WIDTH, PENDING_GLYPH};
use crate::screen_text::models::{ResultLine, ScreenFacts, Tone};
use crate::text::main::cell_len::cell_len;

/// `✗ sqb dev did not start  could not connect   14.7s`, then `Next` and its commands.
#[must_use]
pub fn render_result(result: &ResultLine, facts: &ScreenFacts) -> Vec<String> {
    let (glyph, tone): (&str, Tone) = if result.failed {
        (FAIL_GLYPH, Tone::Error)
    } else {
        (PENDING_GLYPH, Tone::Muted)
    };
    let mut text: String = result.title.clone();
    if !result.detail.is_empty() {
        text = format!("{text}  {}", result.detail);
    }
    if let Some(elapsed) = result.elapsed {
        text = format!("{text}   {}", elapsed_text(elapsed));
    }
    let mut rows: Vec<String> = Vec::new();
    let lead: String = format!("{glyph} ");
    if cell_len(&lead) + cell_len(&text) <= row_width(facts) {
        rows.push(paint(&[span(lead, tone), span(text, Tone::Strong)], facts));
    } else {
        for (index, line) in wrap_words(&text, wrap_width(facts, 2))
            .into_iter()
            .enumerate()
        {
            let first: String = if index == 0 { lead.clone() } else { "  ".to_owned() };
            rows.push(paint(&[span(first, tone), span(line, Tone::Strong)], facts));
        }
    }
    if result.next.is_empty() {
        return rows;
    }
    rows.push(paint(&[span("Next", Tone::Strong)], facts));
    for (command, purpose) in &result.next {
        let lead: String = format!("  {}  ", pad(command, NEXT_COMMAND_WIDTH));
        let column: usize = cell_len(&lead);
        for (index, line) in wrap_words(purpose, wrap_width(facts, column))
            .into_iter()
            .enumerate()
        {
            let first: String = if index == 0 {
                lead.clone()
            } else {
                " ".repeat(column)
            };
            rows.push(paint(&[span(first, Tone::Plain), span(line, Tone::Muted)], facts));
        }
    }
    rows
}
