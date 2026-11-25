//! Refusals grouped by construct and place, laid out as SQLQuack lays out its diagnostics.

use crate::cli_text::models::CliStyle;
use crate::format_command::main::render_help_lines::render_help_lines;
use crate::format_command::main::render_violation_block::render_violation_block;
use crate::format_command::models::LintFinding;
use crate::run_on::constants::{CARET_LINE, EXPANDED_LABEL, NOTE_LABEL, TESTS_NOTE};
use crate::run_on::models::{GroupedRefusal, LocalRefusal, RefusalPlace, RunOnDecision};

/// Each refused construct and place once, in first-seen order, with every item it blocks.
pub(crate) fn grouped_refusals(decisions: &[&RunOnDecision]) -> Vec<GroupedRefusal> {
    let mut places: Vec<RefusalPlace> = Vec::new();
    let mut groups: Vec<GroupedRefusal> = Vec::new();
    for decision in decisions {
        for refusal in &decision.refusals {
            let place: RefusalPlace = refusal_place(refusal);
            match places.iter().position(|seen| *seen == place) {
                Some(index) => {
                    let names: &mut Vec<String> = &mut groups[index].names;
                    if !names.contains(&decision.name) {
                        names.push(decision.name.clone());
                    }
                }
                None => {
                    places.push(place);
                    groups.push(GroupedRefusal {
                        refusal: refusal.clone(),
                        names: vec![decision.name.clone()],
                    });
                }
            }
        }
    }
    groups
}

fn refusal_place(refusal: &LocalRefusal) -> RefusalPlace {
    RefusalPlace {
        path: refusal.path.clone().unwrap_or_default(),
        construct: refusal.construct.clone(),
        line: refusal.line.unwrap_or(0),
        column: refusal.column.unwrap_or(0),
        code: refusal.code.clone().unwrap_or_default(),
    }
}

/// One refusal in SQLQuack's layout, notes, help and the items it blocks: an `error` where it stops them running,
/// a `note` where they run on the warehouse instead.
pub(crate) fn refusal_block(
    refusal: &LocalRefusal,
    names: &[String],
    (errors, style): (bool, CliStyle),
) -> String {
    let message: String = refusal.message.clone().unwrap_or_else(|| refusal.title());
    let code: String = refusal.code.clone().unwrap_or_default();
    let severity: &str = if errors { "error" } else { "note" };
    let labelled: String = if code.is_empty() {
        severity.to_owned()
    } else {
        format!("{severity}[{code}]")
    };
    let header: String = if errors {
        style.error_strong(&labelled)
    } else {
        style.accent_strong(&labelled)
    };
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
                code: code.clone(),
                message,
                fault: true,
                engine: String::new(),
                remediation: None,
                source_line: refusal.snippet.clone(),
            };
            let mut located: Vec<String> = render_violation_block(&finding, style)
                .lines()
                .map(str::to_owned)
                .collect();
            located[0] =
                located[0].replacen(&style.error_strong(&format!("error[{code}]")), &header, 1);
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
            let mut header_lines: Vec<String> = vec![format!("{header}: {message}")];
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
        lines.extend(render_help_lines(help, style));
    }
    if let Some(expanded) = &refusal.expanded_from {
        lines.push(format!("  {} {expanded}", style.muted(EXPANDED_LABEL)));
    }
    lines.push(format!(
        "  {} {}",
        style.muted(TESTS_NOTE),
        names.join(", ")
    ));
    lines.join("\n")
}
