//! Whether this process should send `sqb dev` events.

use crate::dev_events::constants::{DEV_EVENTS_ON, DEV_EVENTS_VARIABLE};

/// True when the parent `sqb dev` set the events variable to `1`.
#[must_use]
pub fn dev_events_enabled() -> bool {
    std::env::var(DEV_EVENTS_VARIABLE).is_ok_and(|value| value == DEV_EVENTS_ON)
}
