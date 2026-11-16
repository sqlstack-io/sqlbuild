//! A change kind as `[changes] counts` and `--changed=` spell it.

use crate::consumption::_helpers::policy::DEFINITION;
use crate::consumption::_helpers::policy::SOURCE_DATA;
use sqlbuild_state::consumption::models::ChangeKind;

/// A change kind as `[changes] counts` and `--changed=` spell it.
pub const fn kind_value(kind: ChangeKind) -> &'static str {
    match kind {
        ChangeKind::Definition => DEFINITION,
        ChangeKind::SourceData => SOURCE_DATA,
    }
}
