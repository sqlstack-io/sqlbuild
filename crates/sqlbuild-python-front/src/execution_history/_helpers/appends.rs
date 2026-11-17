//! Canonical identity, content and envelopes of events about to be appended.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_state::execution_history::main::canonical_event_id::canonical_event_id;
use sqlbuild_state::execution_history::main::prepared_events::prepared_events;
use sqlbuild_state::execution_history::models::{EventSubmission, PreparedEvent};

use crate::execution_history::_helpers::conversion::checked;
use crate::execution_history::_helpers::conversion::envelope_of;
use crate::execution_history::_helpers::conversion::event_id_of;
use crate::execution_history::_helpers::conversion::event_json;

/// The required stable id of a known or opaque event.
pub(crate) fn event_id(event: &Bound<'_, PyAny>) -> PyResult<String> {
    let raw: Option<String> = event_id_of(event)?;
    checked(event.py(), canonical_event_id(raw.as_deref()))
}

/// Deterministic canonical content of an event with a stable id.
pub(crate) fn event_content(event: &Bound<'_, PyAny>) -> PyResult<String> {
    event_id(event)?;
    event_json(event)
}

/// Every event in `events` digested, rejecting ids reused with different content in the batch.
pub(crate) fn prepared_batch(events: &Bound<'_, PyAny>) -> PyResult<Vec<PreparedEvent>> {
    let mut submissions: Vec<EventSubmission> = Vec::new();
    for event in events.try_iter()? {
        let event: Bound<'_, PyAny> = event?;
        submissions.push(EventSubmission {
            event_id: event_id(&event)?,
            content: event_content(&event)?,
            envelope: envelope_of(&event)?,
        });
    }
    checked(events.py(), prepared_events(submissions))
}
