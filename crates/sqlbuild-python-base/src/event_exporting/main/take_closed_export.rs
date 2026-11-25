//! `take_closed_export`: what the sinks of the command scope this thread closed last delivered.

use crate::event_exporting::_helpers::closed_export::CLOSED_EXPORT;
use crate::event_exporting::models::ClosedExport;

/// The newest closed scope's sink counts and teardown failures, once; `None` when no scope closed since.
#[must_use]
pub fn take_closed_export() -> Option<ClosedExport> {
    CLOSED_EXPORT.with(|closed| closed.borrow_mut().take())
}
