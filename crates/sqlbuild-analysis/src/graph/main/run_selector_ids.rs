//! The run IDs, as typed, that the `run:`, `failed:` and `skipped:` selectors name.

use std::collections::BTreeSet;

/// The run IDs, as typed, that the `run:`, `failed:` and `skipped:` selectors name.
#[must_use]
pub fn run_selector_ids(selectors: &[String]) -> BTreeSet<String> {
    crate::graph::_helpers::run_selectors::run_ids(selectors)
}
