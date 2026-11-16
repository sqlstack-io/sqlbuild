//! Why `--changed` holds a node back, as one line for verbose output.

use crate::consumption::_helpers::recorder;
use sqlbuild_state::consumption::models::HeldBack;

/// Why `--changed` holds a node back, as one line for verbose output.
#[inline]
pub fn held_back_text(why: &HeldBack) -> String {
    recorder::held_back_text(why)
}
