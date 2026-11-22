//! Run a frozen ad-hoc build or plan with only the jobs it pins declared.

use crate::job_settings::_helpers::job_catalog::{with_frozen_jobs, with_given_settings};
use crate::job_settings::models::PinnedBuild;

/// Run `run` with only the build's pinned jobs declared, none when it pins none, and its command line's settings given.
pub fn within_pinned_build<T>(pinned: &PinnedBuild, run: impl FnOnce() -> T) -> T {
    with_frozen_jobs(pinned.directory.clone(), pinned.jobs.clone(), || {
        with_given_settings(pinned.given.clone(), run)
    })
}
