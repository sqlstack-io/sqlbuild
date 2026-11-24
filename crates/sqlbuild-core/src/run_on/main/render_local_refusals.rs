//! SQLQuack's refusals as its own diagnostics, placed in the files that hold them.

use std::collections::BTreeMap;

use crate::cli_text::models::CliStyle;
use crate::format_command::_helpers::diagnostics::{help_lines, violation_block};
use crate::format_command::models::LintFinding;
use crate::run_on::constants::{
    CANNOT_RUN_LOCALLY_TITLE, EXPANDED_LABEL, NOTE_LABEL, ON_THE_WAREHOUSE_TITLE, TESTS_NOTE,
};
use crate::run_on::models::{LocalRefusal, RunOnDecision};

/// The caret line's index in a located block: header, `-->`, gutter, source, caret.
const CARET_LINE: usize = 4;

/// `Can't run locally (N tests)` when refusals are errors, else `On the warehouse (N tests)`, then one
/// diagnostic per refused construct and place, with every item it blocks.
#[must_use]
pub fn render_local_refusals(
    decisions: &[RunOnDecision],
    errors: bool,
    (singular, plural): (&str, &str),
    style: CliStyle,
) -> String {
    let refused: Vec<&RunOnDecision> = decisions
        .iter()
        .filter(|decision| !decision.refusals.is_empty())
        .collect();
    if refused.is_empty() {
        return String::new();
    }
    let mut blocks: BTreeMap<(String, String, i64, i64, String), (LocalRefusal, Vec<String>)> =
        BTreeMap::new();
    let mut order: Vec<(String, String, i64, i64, String)> = Vec::new();
    for decision in &refused {
        for refusal in &decision.refusals {
            let key = (
                refusal.path.clone().unwrap_or_default(),
                refusal.construct.clone(),
                refusal.line.unwrap_or(0),
                refusal.column.unwrap_or(0),
                refusal.code.clone().unwrap_or_default(),
            );
            let entry = blocks.entry(key.clone()).or_insert_with(|| {
                order.push(key);
                (refusal.clone(), Vec::new())
            });
            if !entry.1.contains(&decision.name) {
                entry.1.push(decision.name.clone());
            }
        }
    }
    let noun: &str = if refused.len() == 1 { singular } else { plural };
    let title: &str = if errors {
        CANNOT_RUN_LOCALLY_TITLE
    } else {
        ON_THE_WAREHOUSE_TITLE
    };
    let mut text: String = format!("\n{} ({} {noun})\n", style.section(title), refused.len());
    for key in order {
        let (refusal, names) = &blocks[&key];
        text.push('\n');
        text.push_str(&refusal_block(refusal, names, style));
        text.push('\n');
    }
    text
}

/// One refusal in SQLQuack's layout: code and message, place, source line with caret and label,
/// notes, help, then the items it blocks.
fn refusal_block(refusal: &LocalRefusal, names: &[String], style: CliStyle) -> String {
    let message: String = refusal
        .message
        .clone()
        .unwrap_or_else(|| refusal.title());
    let mut lines: Vec<String> = match (&refusal.path, refusal.line, refusal.column) {
        (Some(path), Some(line), Some(column)) => {
            let caret: i64 = refusal.snippet_column.unwrap_or(column);
            let finding: LintFinding = LintFinding {
                file: path.clone(),
                display_path: path.clone(),
                line,
                column: caret,
                end_line: Some(line),
                end_column: Some(caret + refusal.width.max(1)),
                code: refusal.code.clone().unwrap_or_default(),
                message,
                fault: true,
                engine: String::new(),
                remediation: None,
                source_line: refusal.snippet.clone(),
            };
            let mut located: Vec<String> = violation_block(&finding, style)
                .lines()
                .map(str::to_owned)
                .collect();
            if caret != column && located.len() > 1 {
                located[1] = style.muted(&format!(" --> {path}:{line}:{column}"));
            }
            let label: &str = refusal.label.as_deref().unwrap_or(&refusal.reason);
            if refusal.snippet.is_some() && !label.is_empty() && located.len() > CARET_LINE {
                located[CARET_LINE] = format!("{} {label}", located[CARET_LINE]);
            }
            located
        }
        _ => {
            let code: String = refusal.code.clone().unwrap_or_default();
            let header: String = if code.is_empty() {
                format!("{}: {message}", style.error_strong("error"))
            } else {
                format!("{}: {message}", style.error_strong(&format!("error[{code}]")))
            };
            let mut header_lines: Vec<String> = vec![header];
            let label: &str = refusal.label.as_deref().unwrap_or(&refusal.reason);
            if !label.is_empty() && label != message {
                header_lines.push(format!("  {} {label}", style.muted(NOTE_LABEL)));
            }
            header_lines
        }
    };
    for note in &refusal.notes {
        lines.push(format!("  {} {note}", style.muted(NOTE_LABEL)));
    }
    if let Some(help) = &refusal.help {
        lines.extend(help_lines(help, style));
    }
    if let Some(expanded) = &refusal.expanded_from {
        lines.push(format!("  {} {expanded}", style.muted(EXPANDED_LABEL)));
    }
    lines.push(format!("  {} {}", style.muted(TESTS_NOTE), names.join(", ")));
    lines.join("\n")
}
