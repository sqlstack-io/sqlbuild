//! The job and tag dependencies the latest compile of discovered inputs resolved.

use std::sync::PoisonError;

use crate::discovery::_helpers::records::declaration_records::discovered_session;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_python_base::types::NodeSetOutcome;

/// The resolved set dependencies the latest compile of `discovered` kept, if any.
pub fn stored_node_sets(discovered: &Bound<'_, PyAny>) -> PyResult<Option<NodeSetOutcome>> {
    Ok(discovered_session(discovered)?.and_then(|session| {
        session
            .get()
            .node_sets
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }))
}
