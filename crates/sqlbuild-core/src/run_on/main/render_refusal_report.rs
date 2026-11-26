//! The refusals that keep items off DuckDB: one block per construct, each place it is written once.

use crate::cli_text::models::CliStyle;
use crate::run_on::_helpers::refusal_groups::group_refusals;
use crate::run_on::_helpers::refusal_text::{BlockStyle, group_lines};
use crate::run_on::constants::{CANNOT_RUN_LOCALLY_TITLE, ON_THE_WAREHOUSE_TITLE, VERBOSE_HINT};
use crate::run_on::models::RunOnDecision;

/// `Can't run locally (N tests):` with errors, or `On the warehouse (N tests):` with notes; empty with none.
#[must_use]
pub fn render_refusal_report(
    decisions: &[RunOnDecision],
    (errors, verbose): (bool, bool),
    (singular, plural): (&str, &str),
    (columns, style): (usize, CliStyle),
) -> String {
    let refused: Vec<&RunOnDecision> = decisions
        .iter()
        .filter(|decision| !decision.refusals.is_empty())
        .collect();
    if refused.is_empty() {
        return String::new();
    }
    let noun: &str = if refused.len() == 1 { singular } else { plural };
    let title: String = if errors {
        style.warning_strong(&format!(
            "{CANNOT_RUN_LOCALLY_TITLE} ({} {noun}):",
            refused.len()
        ))
    } else {
        style.section(&format!(
            "{ON_THE_WAREHOUSE_TITLE} ({} {noun}):",
            refused.len()
        ))
    };
    let block: BlockStyle<'_> = BlockStyle {
        errors,
        nouns: (singular, plural),
        verbose,
        columns,
        style,
    };
    let mut lines: Vec<String> = vec![String::new(), title];
    let mut hidden: bool = false;
    for group in group_refusals(&refused) {
        let (block_lines, cut) = group_lines(&group, &block);
        lines.push(String::new());
        lines.extend(block_lines);
        hidden |= cut;
    }
    if hidden {
        lines.push(String::new());
        lines.push(style.muted(VERBOSE_HINT));
    }
    format!("{}\n", lines.join("\n"))
}
