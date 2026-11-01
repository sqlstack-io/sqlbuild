//! The strict report: every unproven check, ranked by how many models it leaves partial.

use std::collections::BTreeMap;

use crate::cli_text::_helpers::coverage_actions::{action, reason_detail};
use crate::cli_text::_helpers::summary_text::count_noun;
use crate::cli_text::constants::{NOTICE_DISPLAY_LIMIT, STRICT_REPORT_FOOTER};

/// The report-only strict text for each model's partial-check reasons, most models first, or
/// `None` when every check was complete.
#[must_use]
pub fn strict_report(reasons: &[(String, Vec<String>)]) -> Option<String> {
    let mut groups: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (model, values) in reasons {
        for reason in values {
            groups
                .entry(reason.as_str())
                .or_default()
                .push(model.as_str());
        }
    }
    if groups.is_empty() {
        return None;
    }
    let mut ranked: Vec<(&str, Vec<&str>)> = groups.into_iter().collect();
    for (_, models) in &mut ranked {
        models.sort_unstable();
    }
    ranked.sort_by(|left, right| right.1.len().cmp(&left.1.len()).then(left.0.cmp(right.0)));
    let models: i64 = i64::try_from(reasons.len()).unwrap_or(i64::MAX);
    let checks: i64 = i64::try_from(ranked.len()).unwrap_or(i64::MAX);
    let mut lines: Vec<String> = vec![format!(
        "Strict report: {} unproven across {}:",
        count_noun(checks, "check", None),
        count_noun(models, "model", None)
    )];
    for (rank, (reason, models)) in ranked.iter().enumerate() {
        let shown: Vec<&str> = models.iter().take(NOTICE_DISPLAY_LIMIT).copied().collect();
        let suffix: String = if models.len() > NOTICE_DISPLAY_LIMIT {
            format!(", and {} more", models.len() - NOTICE_DISPLAY_LIMIT)
        } else {
            String::new()
        };
        lines.push(format!(
            "  {}. {} ({}): {}{suffix}; {}.",
            rank + 1,
            reason_detail(reason),
            count_noun(
                i64::try_from(models.len()).unwrap_or(i64::MAX),
                "model",
                None
            ),
            shown.join(", "),
            action(reason)
        ));
    }
    lines.push(STRICT_REPORT_FOOTER.to_owned());
    Some(lines.join("\n"))
}
