//! Register the consumption state functions and classes.

use crate::native_host::_helpers::consumption_state;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the consumption state functions and classes.
#[inline]
pub fn register_consumption_state(module: &Bound<'_, PyModule>) -> PyResult<()> {
    consumption_state::register(module)
}
