//! Encode one event as the line a location child writes.

use crate::dev_events::models::DevEvent;

/// `event` as one JSON line, newline included.
#[must_use]
pub fn dev_event_line(event: &DevEvent) -> String {
    let mut line: String = serde_json::to_string(event).unwrap_or_default();
    line.push('\n');
    line
}
