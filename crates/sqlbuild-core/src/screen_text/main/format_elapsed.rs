//! The elapsed time every screen shows.

use std::time::Duration;

use crate::screen_text::_helpers::rows::elapsed_text;

/// `0.1s` under a minute, `16m 43s` under an hour, else `2h 5m`.
#[must_use]
pub fn format_elapsed(elapsed: Duration) -> String {
    elapsed_text(elapsed)
}
