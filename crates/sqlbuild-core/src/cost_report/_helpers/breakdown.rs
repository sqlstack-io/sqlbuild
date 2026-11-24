//! The status, rate and note lines around a run's cost table.

use crate::cli_text::models::CliStyle;
use crate::cost_report::_helpers::decimal_text::fixed;
use crate::cost_report::constants::{
    COLLECTION_FAILED_STATUS, COMPLETE_STATUS, DEFAULT_RATE_SOURCE, ESTIMATE_NOTE,
    LIST_PRICE_RATE_SOURCE, RATE_ADVICE, RATE_PLACES, RATE_SHEET_RATE_SOURCE, WARNING_STATUSES,
};
use crate::cost_report::models::{CostRun, CostSummary};

/// `_styled_cost_status`.
pub(crate) fn styled_status(style: CliStyle, status: &str) -> String {
    if status == COMPLETE_STATUS {
        return style.value(status);
    }
    if WARNING_STATUSES.contains(&status) {
        return style.warning(status);
    }
    if status == COLLECTION_FAILED_STATUS {
        return style.error(status);
    }
    style.muted(status)
}

/// The credit rate and where it came from; the default and the list price are flagged as estimates.
pub(crate) fn rate_note(style: CliStyle, cost: &CostSummary) -> String {
    let rate: String = fixed(&cost.usd_per_credit, RATE_PLACES);
    match cost.rate_source.as_str() {
        DEFAULT_RATE_SOURCE => style.warning(&format!("default ${rate}/credit")),
        LIST_PRICE_RATE_SOURCE => style.warning(&format!("${rate}/credit est. (list price)")),
        RATE_SHEET_RATE_SOURCE => style.muted(&format!("${rate}/credit (rate sheet)")),
        _ => style.muted(&format!("${rate}/credit")),
    }
}

/// The `Run  <id>` line, when shown.
pub(crate) fn run_line(style: CliStyle, run: &CostRun, show: bool) -> Option<String> {
    show.then(|| format!("{}  {}", style.label("Run"), run.run_id))
}

/// The overflow, message, rate, limitation and method notes after the table.
pub(crate) fn trailer(style: CliStyle, run: &CostRun, limit: Option<usize>) -> Vec<String> {
    let cost: &CostSummary = &run.cost;
    let mut lines: Vec<String> = Vec::new();
    if let Some(limit) = limit
        && cost.resources.len() > limit
    {
        let total: usize = cost.resources.len();
        lines.push(style.muted(&format!(
            "Showing {limit} of {total} by estimated cost; `sqb cost {}` for all models ({} more).",
            run.run_id,
            total - limit
        )));
    }
    if let Some(message) = cost.message.as_ref().filter(|message| !message.is_empty()) {
        lines.push(style.muted(message));
    }
    if [DEFAULT_RATE_SOURCE, LIST_PRICE_RATE_SOURCE].contains(&cost.rate_source.as_str()) {
        lines.push(style.warning(RATE_ADVICE));
    }
    lines.extend(
        cost.limitations
            .iter()
            .map(|limitation| style.warning(limitation)),
    );
    lines.push(style.muted(ESTIMATE_NOTE));
    lines
}
