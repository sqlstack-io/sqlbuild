//! Register the events functions and classes.

use crate::state_rows::_helpers::events::events;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the events functions and classes.
#[inline]
pub fn register_events(module: &Bound<'_, PyModule>) -> PyResult<()> {
    events::register(module)
}
