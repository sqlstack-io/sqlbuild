//! The one box of the `sqb dev` screen: what is ready and where.

use crate::screen_text::_helpers::rows::{pad, paint, span, wrap_words};
use crate::screen_text::constants::{
    BOX_LABEL_WIDTH, BOX_MARGIN, BOX_MIN_WIDTH, RULE_CHAR, SEPARATOR,
};
use crate::screen_text::models::{BoxRow, ScreenFacts, Segment, Tone};
use crate::text::main::cell_len::cell_len;

/// `╭─ ready ─╮`, one labelled row each, wrapped between parts, then `╰─╯`.
#[must_use]
pub fn render_ready_box(rows: &[BoxRow], facts: &ScreenFacts) -> Vec<String> {
    let width: usize = facts
        .columns
        .saturating_sub(BOX_MARGIN)
        .max(BOX_MIN_WIDTH);
    let inner: usize = width - 4;
    let value_width: usize = inner.saturating_sub(BOX_LABEL_WIDTH).max(1);
    let title: &str = "\u{256d}\u{2500} ready ";
    let top: String = format!(
        "{title}{}\u{256e}",
        rule(width.saturating_sub(cell_len(title) + 1))
    );
    let mut lines: Vec<String> = vec![facts.style.muted(&top)];
    for row in rows {
        for (index, value) in value_lines(&row.parts, value_width).into_iter().enumerate() {
            let label: &str = if index == 0 { &row.label } else { "" };
            let tone: Tone = if row.link.is_some() {
                Tone::Link
            } else {
                Tone::Plain
            };
            let padding: usize = value_width.saturating_sub(cell_len(&value));
            let segments: Vec<Segment> = vec![
                span("\u{2502} ", Tone::Muted),
                span(pad(label, BOX_LABEL_WIDTH), Tone::Muted),
                Segment {
                    text: value,
                    tone,
                    link: row.link.clone(),
                },
                span(" ".repeat(padding), Tone::Plain),
                span(" \u{2502}", Tone::Muted),
            ];
            lines.push(paint(&segments, facts));
        }
    }
    let bottom: String = format!("\u{2570}{}\u{256f}", rule(width - 2));
    lines.push(facts.style.muted(&bottom));
    lines
}

/// `parts` joined with ` · `, starting a new line between parts that would not fit.
fn value_lines(parts: &[String], width: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut line: String = String::new();
    for part in parts.iter().filter(|part| !part.is_empty()) {
        if line.is_empty() {
            line.clone_from(part);
        } else if cell_len(&line) + cell_len(SEPARATOR) + cell_len(part) <= width {
            line = format!("{line}{SEPARATOR}{part}");
        } else {
            lines.push(std::mem::take(&mut line));
            line.clone_from(part);
        }
        if cell_len(&line) > width {
            let mut wrapped: Vec<String> = wrap_words(&line, width);
            line = wrapped.pop().unwrap_or_default();
            lines.extend(wrapped);
        }
    }
    lines.push(line);
    lines
}

fn rule(cells: usize) -> String {
    std::iter::repeat_n(RULE_CHAR, cells).collect()
}
