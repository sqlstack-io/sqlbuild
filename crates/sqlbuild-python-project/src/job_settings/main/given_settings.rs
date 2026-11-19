//! The settings the running command's own command line gives.

use std::collections::BTreeSet;

use crate::job_settings::_helpers::job_catalog;

/// The settings the running command's command line gives, which replace named jobs' settings.
#[inline]
pub fn given_settings() -> BTreeSet<String> {
    job_catalog::given_settings()
}
