//! Table rows for the `sqb dev` event and control lines.

use crate::dev_events::models::{DevControl, DevEvent, SourcePosition};

/// An event and whether its encoded line reads back as itself.
pub(super) struct RoundTripTestCase {
    pub(super) description: &'static str,
    pub(super) event: DevEvent,
    pub(super) expected_newlines: usize,
}

/// A line from the location child and the event it parses as, if any.
pub(super) struct ParseTestCase {
    pub(super) description: &'static str,
    pub(super) line: String,
    pub(super) expected_event: Option<DevEvent>,
}

/// A line sent to the location child and the control it parses as, if any.
pub(super) struct ControlTestCase {
    pub(super) description: &'static str,
    pub(super) line: String,
    pub(super) expected_control: Option<DevControl>,
}

/// An error message and the first source position in it.
pub(super) struct PositionTestCase {
    pub(super) description: &'static str,
    pub(super) message: &'static str,
    pub(super) expected_position: Option<SourcePosition>,
}
