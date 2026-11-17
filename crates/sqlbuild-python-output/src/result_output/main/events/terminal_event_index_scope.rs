//! Install and reliably restore one invocation terminal event index.

use crate::result_output::models::TerminalEventIndex;
use crate::result_output::models::TerminalEventIndexScope;
use pyo3::prelude::Py;
use pyo3::pyfunction;
use std::sync::Mutex;

/// Install and reliably restore one invocation terminal event index.
#[pyfunction]
pub fn terminal_event_index_scope(index: Py<TerminalEventIndex>) -> TerminalEventIndexScope {
    TerminalEventIndexScope {
        index,
        token: Mutex::new(None),
    }
}
