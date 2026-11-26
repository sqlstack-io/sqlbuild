//! Whether a command's selectors name an audit or a past run, so its graph needs them attached.

use sqlbuild_analysis::graph::constants::{AUDIT_KIND, RUN_SELECTOR_KINDS};
use sqlbuild_analysis::graph::main::parse_selector::parse_selector;
use sqlbuild_analysis::graph::models::ParsedSelector;

/// Whether a part of `selectors` is an `audit:` or a past-run selector.
#[must_use]
pub fn names_audits_or_runs(selectors: &[String]) -> bool {
    selectors
        .iter()
        .flat_map(|selector| selector.split(char::is_whitespace))
        .flat_map(|token| token.split(','))
        .any(|part| {
            matches!(parse_selector(part), Ok(ParsedSelector::Kind { kind, .. })
                if kind == AUDIT_KIND || RUN_SELECTOR_KINDS.contains(&kind.as_str()))
        })
}
