//! The statements one part of an invocation finished, from summaries taken before and after it.

use crate::progress::models::StatementSummary;

/// The statements counted in `after` and not yet in `before`.
#[must_use]
pub fn statement_summary_delta(
    before: &StatementSummary,
    after: &StatementSummary,
) -> StatementSummary {
    let mut delta: StatementSummary = StatementSummary::default();
    for (kind, count) in &after.by_kind {
        let earlier: u64 = before.by_kind.get(kind).copied().unwrap_or(0);
        if *count > earlier {
            delta.by_kind.insert(kind.clone(), count - earlier);
        }
    }
    delta.query_ids = after
        .query_ids
        .iter()
        .filter(|query_id| !before.query_ids.contains(query_id))
        .cloned()
        .collect();
    delta
}
