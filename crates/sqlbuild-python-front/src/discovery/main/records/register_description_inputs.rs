//! Register the description inputs functions and classes.

use crate::discovery::_helpers::records::description_inputs;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the description inputs functions and classes.
#[inline]
pub fn register_description_inputs(module: &Bound<'_, PyModule>) -> PyResult<()> {
    description_inputs::register(module)
}
