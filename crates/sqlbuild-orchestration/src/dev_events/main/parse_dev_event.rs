//! Tell an event line from any other line a location child writes.

use crate::dev_events::constants::EVENT_LINE_PREFIX;
use crate::dev_events::models::DevEvent;

/// The event on `line`, or `None` for reply lines, publication records and malformed events.
#[must_use]
pub fn parse_dev_event(line: &str) -> Option<DevEvent> {
    let trimmed: &str = line.trim_start();
    if !trimmed.starts_with(EVENT_LINE_PREFIX) {
        return None;
    }
    let Ok(event) = serde_json::from_str(trimmed) else {
        return None;
    };
    Some(event)
}
