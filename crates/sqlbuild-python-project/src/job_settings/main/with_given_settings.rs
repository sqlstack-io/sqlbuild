//! Run one command whose command line gives some settings itself.

use std::collections::BTreeSet;

use crate::job_settings::_helpers::job_catalog;

/// Run one command whose command line gives `given` settings, restoring the enclosing ones after.
#[inline]
pub fn with_given_settings<T>(given: BTreeSet<String>, run: impl FnOnce() -> T) -> T {
    job_catalog::with_given_settings(given, run)
}
