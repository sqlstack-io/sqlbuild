//! The `LABEL=value` counts the footer and the summary line share.

use crate::cli_text::models::SummaryCount;
use crate::run_output::constants::{
    FAIL_LABEL, NOT_RUN_LABEL, PASS_LABEL, TIMEOUT_LABEL, TOTAL_LABEL,
};
use crate::run_output::models::RunCounts;

/// `PASS` and `FAIL`, then `TIMEOUT` and `NOT_RUN` when there are any, then `TOTAL` when asked.
pub(crate) fn summary_counts(counts: &RunCounts, with_total: bool) -> Vec<SummaryCount> {
    let mut shown: Vec<SummaryCount> = vec![
        SummaryCount::of(PASS_LABEL, counts.pass),
        SummaryCount::of(FAIL_LABEL, counts.fail),
    ];
    if counts.timeout > 0 {
        shown.push(SummaryCount::of(TIMEOUT_LABEL, counts.timeout));
    }
    if counts.not_run > 0 {
        shown.push(SummaryCount::of(NOT_RUN_LABEL, counts.not_run));
    }
    if with_total {
        shown.push(SummaryCount::of(TOTAL_LABEL, counts.total));
    }
    shown
}
