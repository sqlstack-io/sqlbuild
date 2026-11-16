//! Why `--changed` runs a node, as one line for verbose output.

use crate::consumption::_helpers::recorder;
use sqlbuild_state::consumption::models::RunReason;

/// Why `--changed` runs a node, as one line for verbose output.
#[inline]
pub fn run_reason_text(reason: &RunReason) -> String {
    recorder::run_reason_text(reason)
}
