//! SQLQuack's refusals as its own diagnostics, placed in the files that hold them.

use crate::cli_text::models::CliStyle;
use crate::run_on::_helpers::refusal_blocks::{grouped_refusals, refusal_block};
use crate::run_on::constants::{CANNOT_RUN_LOCALLY_TITLE, ON_THE_WAREHOUSE_TITLE};
use crate::run_on::models::{GroupedRefusal, RunOnDecision};

/// A titled section with one diagnostic per refused construct and place, and the items it blocks.
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
    let noun: &str = if refused.len() == 1 { singular } else { plural };
    let title: &str = if errors {
        CANNOT_RUN_LOCALLY_TITLE
    } else {
        ON_THE_WAREHOUSE_TITLE
    };
    let mut text: String = format!("\n{} ({} {noun})\n", style.section(title), refused.len());
    for group in grouped_refusals(&refused) {
        let GroupedRefusal { refusal, names } = group;
        text.push('\n');
        text.push_str(&refusal_block(&refusal, &names, style));
        text.push('\n');
    }
    text
}
