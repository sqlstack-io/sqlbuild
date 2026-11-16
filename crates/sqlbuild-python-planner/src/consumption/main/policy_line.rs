//! One line saying what the policy counts and how far it reaches, as build and plan print it.

use crate::consumption::_helpers::policy;
use sqlbuild_state::consumption::models::ChangedPolicy;

/// One line saying what the policy counts and how far it reaches, as build and plan print it.
#[inline]
pub fn policy_line(policy: &ChangedPolicy) -> String {
    policy::policy_line(policy)
}
