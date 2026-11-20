//! A problem block: what went wrong, where, why, and how to fix it, wrapped in full.

use crate::screen_text::_helpers::rows::{pad, paint, row_width, span, wrap_width, wrap_words};
use crate::screen_text::constants::{
    FAIL_GLYPH, PROBLEM_LABEL_INDENT, PROBLEM_TEXT_COLUMN, WARN_GLYPH,
};
use crate::screen_text::models::{Problem, ProblemLevel, ScreenFacts, Segment, Tone};
use crate::text::main::cell_len::cell_len;

/// `✗ error  title  code`, then `at`, `why` and `fix` with their text under one another.
#[must_use]
pub fn render_problem(problem: &Problem, facts: &ScreenFacts) -> Vec<String> {
    let (glyph, level, tone): (&str, &str, Tone) = match problem.level {
        ProblemLevel::Error => (FAIL_GLYPH, "error", Tone::Error),
        ProblemLevel::Warning => (WARN_GLYPH, "warn", Tone::Warning),
    };
    let lead: String = pad(&format!("{glyph} {level}"), PROBLEM_TEXT_COLUMN);
    let mut rows: Vec<String> = title_rows(problem, (&lead, tone), facts);
    for (index, place) in problem.at.iter().enumerate() {
        let label: &str = if index == 0 { "at" } else { "" };
        rows.push(paint(
            &[span(field_lead(label), Tone::Muted), place.clone()],
            facts,
        ));
    }
    for (label, text) in [("why", &problem.why), ("fix", &problem.fix)] {
        if text.trim().is_empty() {
            continue;
        }
        for (index, line) in wrap_words(text, wrap_width(facts, PROBLEM_TEXT_COLUMN))
            .into_iter()
            .enumerate()
        {
            let label: &str = if index == 0 { label } else { "" };
            rows.push(paint(
                &[span(field_lead(label), Tone::Muted), span(line, Tone::Plain)],
                facts,
            ));
        }
    }
    rows
}

/// The title line with the code at its right edge, or the title wrapped with the code after it.
fn title_rows(problem: &Problem, (lead, tone): (&str, Tone), facts: &ScreenFacts) -> Vec<String> {
    let width: usize = row_width(facts);
    let used: usize = cell_len(lead) + cell_len(&problem.title);
    if used + 2 + cell_len(&problem.code) <= width {
        let gap: usize = width - used - cell_len(&problem.code);
        let gap: usize = if problem.code.is_empty() { 0 } else { gap };
        return vec![paint(
            &[
                span(lead, tone),
                span(problem.title.as_str(), Tone::Strong),
                span(" ".repeat(gap), Tone::Plain),
                span(problem.code.as_str(), Tone::Muted),
            ],
            facts,
        )];
    }
    let text: String = if problem.code.is_empty() {
        problem.title.clone()
    } else {
        format!("{}  {}", problem.title, problem.code)
    };
    wrap_words(&text, wrap_width(facts, PROBLEM_TEXT_COLUMN))
        .into_iter()
        .enumerate()
        .map(|(index, line)| {
            let first: Segment = if index == 0 {
                span(lead, tone)
            } else {
                span(" ".repeat(PROBLEM_TEXT_COLUMN), Tone::Plain)
            };
            paint(&[first, span(line, Tone::Strong)], facts)
        })
        .collect()
}

/// `  why    `: the label indented and padded to the text column.
fn field_lead(label: &str) -> String {
    pad(
        &format!("{}{label}", " ".repeat(PROBLEM_LABEL_INDENT)),
        PROBLEM_TEXT_COLUMN,
    )
}
