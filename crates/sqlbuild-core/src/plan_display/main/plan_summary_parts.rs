//! What a plan will run, in the parts its `Plan ready` line and its `planned` event share.

use crate::plan_display::models::PlanSummaryCounts;

/// Each non-zero count with its noun, such as `3 selected` and `1 Python node`; `0 selected` when all are zero.
#[must_use]
pub fn plan_summary_parts(counts: &PlanSummaryCounts) -> Vec<String> {
    let noun = |count: usize, one: &str, many: &str| {
        format!("{count} {}", if count == 1 { one } else { many })
    };
    let action: &str = if counts.reloads_sources {
        "reload"
    } else {
        "load"
    };
    let parts: Vec<String> = [
        (counts.full_refresh, "full refresh".to_owned()),
        (counts.selected > 0, format!("{} selected", counts.selected)),
        (
            counts.sources > 0,
            format!("{} to {action}", noun(counts.sources, "source", "sources")),
        ),
        (
            counts.loads > 0,
            format!("{} to {action}", noun(counts.loads, "load", "loads")),
        ),
        (
            counts.python > 0,
            noun(counts.python, "Python node", "Python nodes"),
        ),
        (counts.audits > 0, noun(counts.audits, "audit", "audits")),
        (
            counts.audits_not_run > 0,
            format!("{} not run", noun(counts.audits_not_run, "audit", "audits")),
        ),
    ]
    .into_iter()
    .filter_map(|(shown, part)| shown.then_some(part))
    .collect();
    if parts.is_empty() {
        return vec!["0 selected".to_owned()];
    }
    parts
}
