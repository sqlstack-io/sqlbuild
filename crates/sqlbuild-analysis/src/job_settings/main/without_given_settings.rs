//! The declared jobs less the settings the command line gives, which replace theirs.

use std::collections::{BTreeMap, BTreeSet};

use crate::job_settings::models::DeclaredRun;

/// `declared` with each setting named in `given` removed from every job, so it never conflicts.
#[must_use]
pub fn without_given_settings(
    mut declared: BTreeMap<String, DeclaredRun>,
    given: &BTreeSet<String>,
) -> BTreeMap<String, DeclaredRun> {
    for run in declared.values_mut() {
        if let DeclaredRun::Job { settings, .. } = run {
            settings.retain(|setting, _| !given.contains(setting));
        }
    }
    declared
}
