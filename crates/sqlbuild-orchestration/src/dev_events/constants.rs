//! How `sqb dev` asks its location child for events.

/// Set to `1` on the location child, and inherited by its runtime, to send events on stdout.
pub const DEV_EVENTS_VARIABLE: &str = "SQLBUILD_DEV_EVENTS";
/// Every event line starts with this, so readers can tell it from a reply line.
pub const EVENT_LINE_PREFIX: &str = "{\"event\":";
/// Set by `sqb dev` on its foreground `sqb debug --json`: the file the report is written to instead of stdout, so nothing a warehouse client prints can be read as the report.
pub const DEBUG_REPORT_VARIABLE: &str = "SQLBUILD_DEBUG_REPORT_FILE";
/// The events variable's value that turns events on.
pub const DEV_EVENTS_ON: &str = "1";
/// Files an error position may point into.
pub(crate) const SOURCE_SUFFIXES: [&str; 5] = [".sql", ".py", ".yml", ".yaml", ".toml"];
/// Punctuation around a position in a message, such as quotes or brackets.
pub(crate) const POSITION_WRAPPERS: [char; 9] = ['(', ')', '[', ']', ',', ';', '\'', '"', '`'];
