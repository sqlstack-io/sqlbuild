//! Encode one control request as the line `sqb dev` writes to its location child.

use crate::dev_events::models::DevControl;

/// `control` as one JSON line, newline included.
#[must_use]
pub fn dev_control_line(control: DevControl) -> String {
    let mut line: String = serde_json::to_string(&control).unwrap_or_default();
    line.push('\n');
    line
}
