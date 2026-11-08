//! `counts_toward_exit`: whether a lint finding can make a command fail.

use crate::lint_text::constants::LINT_UNAVAILABLE_CODE;

/// Whether a finding with `code` counts toward a command's exit status; SQL lint being unavailable for a dialect never does.
pub fn counts_toward_exit(code: &str) -> bool {
    code != LINT_UNAVAILABLE_CODE
}
