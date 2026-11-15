//! How `sqb dev` asks its location child for events.

/// Set to `1` on the location child, and inherited by its runtime, to send events on stdout.
pub const DEV_EVENTS_VARIABLE: &str = "SQLBUILD_DEV_EVENTS";
/// Every event line starts with this, so readers can tell it from a reply line.
pub const EVENT_LINE_PREFIX: &str = "{\"event\":";
