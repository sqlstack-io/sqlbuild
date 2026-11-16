//! A reach as `[changes] reach` and `--reach` spell it.

use crate::consumption::_helpers::policy;
use sqlbuild_state::consumption::models::Reach;

/// A reach as `[changes] reach` and `--reach` spell it.
#[inline]
pub fn reach_value(reach: &Reach) -> &'static str {
    policy::reach_value(reach)
}
