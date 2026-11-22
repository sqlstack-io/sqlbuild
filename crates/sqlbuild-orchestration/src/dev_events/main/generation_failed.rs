//! The event for a generation that failed to compile or publish.

use crate::dev_events::_helpers::source_position::source_position;
use crate::dev_events::models::DevEvent;

/// `generation` failed with `message`, its `code` and `help`, pointing at the first source position in it.
#[must_use]
pub fn generation_failed(
    generation: i64,
    message: String,
    (code, help): (Option<String>, Option<String>),
) -> DevEvent {
    DevEvent::Failed {
        generation,
        position: source_position(&message),
        message,
        code,
        help,
    }
}
