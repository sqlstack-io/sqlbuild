//! Record the discovered sets whose items depend on a tag no node carries.

use std::sync::PoisonError;

use crate::discovered_sets::_helpers::state::TAG_FAILURES;

/// Replace the recorded failures with `failures`, each a set and the error naming its item and tag.
pub fn record_tag_failures(failures: Vec<(String, String)>) {
    *TAG_FAILURES.lock().unwrap_or_else(PoisonError::into_inner) = failures.into_iter().collect();
}
