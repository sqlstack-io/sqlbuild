//! One stable line of the plain `sqb dev` output.

use std::time::Duration;

use crate::screen_text::_helpers::rows::elapsed_text;
use crate::screen_text::constants::PLAIN_PREFIX;

/// `sqlbuild dev: <message>`, followed by ` (<elapsed>)` when timed.
#[must_use]
pub fn format_plain_line(message: &str, elapsed: Option<Duration>) -> String {
    match elapsed {
        Some(elapsed) => format!("{PLAIN_PREFIX}{message} ({})", elapsed_text(elapsed)),
        None => format!("{PLAIN_PREFIX}{message}"),
    }
}
