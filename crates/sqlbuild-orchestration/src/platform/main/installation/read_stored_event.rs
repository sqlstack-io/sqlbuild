//! A reader with its own page of platform rows reads each on its own, so one that can't be read never stops it.

use crate::platform::_helpers::health::event_history::read_event;
use crate::platform::models::{PlatformEventEnvelope, StoredPlatformEvent, UnreadableRow};

/// The row's event, or what is known about it and why it can't be read.
pub(crate) fn read_stored_event(
    row: StoredPlatformEvent,
) -> Result<PlatformEventEnvelope, Box<UnreadableRow>> {
    read_event(row)
}
