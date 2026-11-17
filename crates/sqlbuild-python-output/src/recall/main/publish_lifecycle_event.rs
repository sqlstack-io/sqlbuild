//! Queue `event` in `session`; an event that cannot be encoded is skipped.

use crate::recall::_helpers::sinks::lifecycle_event_object;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_recall::writer::models::RecallSession;

/// Queue `event` in `session`; an event that cannot be encoded is skipped.
pub fn publish_lifecycle_event(
    py: Python<'_>,
    session: &RecallSession,
    event: &Bound<'_, PyAny>,
) -> PyResult<()> {
    if let Some(object) = lifecycle_event_object(py, event)? {
        session.publish_event_object(object);
    }
    Ok(())
}
