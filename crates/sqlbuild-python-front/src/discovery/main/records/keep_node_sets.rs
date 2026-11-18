//! Keep a compile's resolved job and tag dependencies with the discovered inputs it compiled.

use std::sync::PoisonError;

use crate::discovery::_helpers::records::declaration_records::discovered_session;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_python_base::types::NodeSetOutcome;

/// Keep `outcome` in the discovery session `discovered` was read in, when it has one.
pub fn keep_node_sets(
    discovered: &Bound<'_, PyAny>,
    outcome: Option<NodeSetOutcome>,
) -> PyResult<()> {
    if let Some(session) = discovered_session(discovered)? {
        *session
            .get()
            .node_sets
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = outcome;
    }
    Ok(())
}
