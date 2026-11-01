//! One actionable notice summarizing partial semantic checks, without counting as a warning.

use std::collections::BTreeMap;

use crate::cli_text::_helpers::coverage_actions::{action, reason_detail};
use crate::cli_text::_helpers::summary_text::count_noun;

use crate::cli_text::constants::NOTICE_DISPLAY_LIMIT;

/// The notice for each model's partial-check reasons, or `None` when every check was complete.
#[must_use]
pub fn semantic_coverage_notice(reasons: &[(String, Vec<String>)]) -> Option<String> {
    if reasons.is_empty() {
        return None;
    }
    let mut by_model: Vec<&(String, Vec<String>)> = reasons.iter().collect();
    by_model.sort_by(|left, right| left.0.cmp(&right.0));
    let mut groups: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (model, values) in by_model {
        for reason in values {
            groups
                .entry(reason.as_str())
                .or_default()
                .push(model.as_str());
        }
    }
    let count: i64 = i64::try_from(reasons.len()).unwrap_or(i64::MAX);
    let mut lines: Vec<String> = vec![format!(
        "Semantic checks were partial for {}:",
        count_noun(count, "model", None)
    )];
    for (reason, models) in groups.iter().take(NOTICE_DISPLAY_LIMIT) {
        let suffix: String = if models.len() > NOTICE_DISPLAY_LIMIT {
            format!(", and {} more", models.len() - NOTICE_DISPLAY_LIMIT)
        } else {
            String::new()
        };
        let detail: String = reason_detail(reason);
        let shown: Vec<&str> = models.iter().take(NOTICE_DISPLAY_LIMIT).copied().collect();
        lines.push(format!(
            "  {}{suffix}: {detail}; {}.",
            shown.join(", "),
            action(reason)
        ));
    }
    if groups.len() > NOTICE_DISPLAY_LIMIT {
        lines.push(format!(
            "  and {} more reasons; run `sqb compile --json` for the full model/reason list.",
            groups.len() - NOTICE_DISPLAY_LIMIT
        ));
    }
    Some(lines.join("\n"))
}
