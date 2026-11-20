//! A warehouse client's sign-in prompt, drawn above the live steps until it is answered.

use crate::screen_text::_helpers::rows::{break_cells, fit_row, paint, row_width, span};
use crate::screen_text::constants::{
    PROBLEM_LABEL_INDENT, PROBLEM_TEXT_COLUMN, PROMPT_GLYPH, RULE_CHAR, SECONDS_PER_MINUTE,
};
use crate::screen_text::models::{PromptBlock, ScreenFacts, Tone};

/// `◌ sign in  title`, the client's text broken to the terminal so nothing is cut, the wait, and a rule.
#[must_use]
pub fn render_prompt(block: &PromptBlock, facts: &ScreenFacts) -> Vec<String> {
    let width: usize = row_width(facts);
    let mut rows: Vec<String> = vec![fit_row(
        vec![
            span(format!("{PROMPT_GLYPH} sign in  "), Tone::Warning),
            span(block.title.as_str(), Tone::Strong),
        ],
        width,
        facts,
    )];
    let indent: String = " ".repeat(PROBLEM_LABEL_INDENT);
    let text: &str = block.text.trim_matches(['\n', '\r']);
    if !text.is_empty() {
        for row in break_cells(text, width.saturating_sub(PROBLEM_LABEL_INDENT)) {
            rows.push(paint(
                &[span(indent.as_str(), Tone::Plain), span(row, Tone::Plain)],
                facts,
            ));
        }
    }
    let seconds: u64 = block.waited.as_secs();
    rows.push(fit_row(
        vec![
            span(
                format!("{indent}{:<1$}", "wait", PROBLEM_TEXT_COLUMN - PROBLEM_LABEL_INDENT),
                Tone::Muted,
            ),
            span(
                format!(
                    "{}:{:02} \u{b7} Ctrl+C cancels and stops sqb dev",
                    seconds / SECONDS_PER_MINUTE,
                    seconds % SECONDS_PER_MINUTE
                ),
                Tone::Muted,
            ),
        ],
        width,
        facts,
    ));
    rows.push(facts.style.muted(&std::iter::repeat_n(RULE_CHAR, width).collect::<String>()));
    rows
}
