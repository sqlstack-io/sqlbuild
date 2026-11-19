//! Whether a selector can name a job.

use sqlbuild_analysis::graph::main::parse_selector::parse_selector;
use sqlbuild_analysis::graph::models::ParsedSelector;

use crate::compiled_project::constants::{JOB_SELECTOR_KINDS, SELECTOR_INTERSECTION};

/// Whether a selector has an atom that can name a job: `job:<name>`, or a bare name.
pub(crate) fn could_name_job(selector: &str) -> bool {
    selector
        .split_whitespace()
        .flat_map(|token| token.split(SELECTOR_INTERSECTION))
        .any(|atom| match parse_selector(atom) {
            Ok(ParsedSelector::Kind { kind, .. }) => JOB_SELECTOR_KINDS.contains(&kind.as_str()),
            _ => false,
        })
}
