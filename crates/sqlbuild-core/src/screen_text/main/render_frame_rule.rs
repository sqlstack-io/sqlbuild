//! The dim rule that frames a client's own terminal output.

use crate::screen_text::_helpers::rows::{render_row, span};
use crate::screen_text::constants::RULE_CHAR;
use crate::screen_text::models::{ScreenFacts, Tone};
use crate::text::main::cell_len::cell_len;

/// `── <label> ────` across the screen, or a plain rule without a label.
#[must_use]
pub fn render_frame_rule(label: Option<&str>, facts: &ScreenFacts) -> String {
    let width: usize = facts.columns.saturating_sub(facts.indent);
    let opening: String = match label {
        Some(label) => format!("{RULE_CHAR}{RULE_CHAR} {label} "),
        None => String::new(),
    };
    let fill: String = RULE_CHAR
        .to_string()
        .repeat(width.saturating_sub(cell_len(&opening)));
    render_row(vec![span(format!("{opening}{fill}"), Tone::Muted)], facts)
}
