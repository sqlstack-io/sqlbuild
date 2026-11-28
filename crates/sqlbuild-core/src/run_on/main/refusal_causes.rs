//! The causes that keep items off DuckDB, as a run's header counts them.

use crate::run_on::_helpers::refusal_groups::group_refusals;
use crate::run_on::models::RunOnDecision;
use crate::run_output::models::HeaderCause;

/// One cause per refused construct, most items first, with its code and how many models it is in.
#[must_use]
pub fn refusal_causes(decisions: &[RunOnDecision]) -> Vec<HeaderCause> {
    let refused: Vec<&RunOnDecision> = decisions
        .iter()
        .filter(|decision| !decision.refusals.is_empty())
        .collect();
    group_refusals(&refused)
        .iter()
        .map(|group| {
            let mut detail: Vec<String> = group.refusal.code.iter().cloned().collect();
            let models: usize = group.models();
            if models > 0 {
                detail.push(if models == 1 {
                    "1 model".to_owned()
                } else {
                    format!("{models} models")
                });
            }
            HeaderCause {
                label: group.label(),
                count: group.items.len(),
                detail: detail.join(" \u{00b7} "),
            }
        })
        .collect()
}
