//! A reach as `[changes] reach` and `--reach` spell it.

use crate::consumption::_helpers::policy::REACH_ALL;
use crate::consumption::_helpers::policy::REACH_JOB;
use sqlbuild_state::consumption::models::Reach;

/// A reach as `[changes] reach` and `--reach` spell it.
pub const fn reach_value(reach: &Reach) -> &'static str {
    match reach {
        Reach::Job(_) => REACH_JOB,
        Reach::All => REACH_ALL,
    }
}
