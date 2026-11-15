//! Read one control request from `sqb dev`.

use crate::dev_events::models::DevControl;

/// The control request on `line`, or `None` when it is not one.
#[must_use]
pub fn parse_dev_control(line: &str) -> Option<DevControl> {
    serde_json::from_str(line.trim()).ok()
}
