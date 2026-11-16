//! Send one event to a `sqb dev` parent.

use std::io::Write;

use crate::dev_events::main::dev_event_line::dev_event_line;
use crate::dev_events::main::dev_events_enabled::dev_events_enabled;
use crate::dev_events::models::DevEvent;

/// Write `event` to stdout ahead of the reply line; silent outside `sqb dev`.
pub fn emit_dev_event(event: &DevEvent) -> std::io::Result<()> {
    if !dev_events_enabled() {
        return Ok(());
    }
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(dev_event_line(event).as_bytes())?;
    stdout.flush()
}
