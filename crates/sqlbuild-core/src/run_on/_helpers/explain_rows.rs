//! `[N] reason  where`: each place a construct is written and how many items it keeps off DuckDB.

use std::cmp::Reverse;

use crate::run_on::_helpers::refusal_groups::group_refusals;
use crate::run_on::models::{LocalRefusal, RunOnDecision};
use crate::run_output::models::ExplainRow;

/// One row per origin, most items first: the construct, then the macro and its file, or the file position.
pub(crate) fn refusal_explain_rows(decisions: &[RunOnDecision]) -> Vec<ExplainRow> {
    let refused: Vec<&RunOnDecision> = decisions
        .iter()
        .filter(|decision| !decision.refusals.is_empty())
        .collect();
    let mut rows: Vec<ExplainRow> = Vec::new();
    for group in group_refusals(&refused) {
        for origin in &group.origins {
            rows.push(ExplainRow {
                count: origin.items.len(),
                reason: group.label(),
                place: place(&origin.refusal, group.unplaced),
            });
        }
    }
    rows.sort_by_key(|row| Reverse(row.count));
    rows
}

fn place(refusal: &LocalRefusal, unplaced: bool) -> String {
    if unplaced {
        return refusal.construct.clone();
    }
    let position: Option<String> = match (&refusal.macro_place, &refusal.path, refusal.line) {
        (Some(place), _, _) => Some(format!("{}:{}", place.path, place.line)),
        (None, Some(path), Some(line)) => Some(format!("{path}:{line}")),
        _ => None,
    };
    match (&refusal.macro_name, position) {
        (Some(name), Some(position)) if refusal.macro_place.is_some() => {
            format!("{name}  {position}")
        }
        (Some(name), _) => name.clone(),
        (None, Some(position)) => position,
        (None, None) => String::new(),
    }
}
