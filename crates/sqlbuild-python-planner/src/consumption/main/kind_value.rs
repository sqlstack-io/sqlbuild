//! A change kind as `[changes] counts` and `--changed=` spell it.

use crate::consumption::_helpers::policy;
use sqlbuild_state::consumption::models::ChangeKind;

/// A change kind as `[changes] counts` and `--changed=` spell it.
#[inline]
pub fn kind_value(kind: ChangeKind) -> &'static str {
    policy::kind_value(kind)
}
