//! Register the envelopes functions and classes.

use crate::lifecycle_events::_helpers::envelopes;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the envelopes functions and classes.
#[inline]
pub fn register_envelopes(module: &Bound<'_, PyModule>) -> PyResult<()> {
    envelopes::register(module)
}
