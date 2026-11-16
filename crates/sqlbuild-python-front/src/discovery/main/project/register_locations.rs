//! Register the locations functions and classes.

use crate::discovery::_helpers::project::locations;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the locations functions and classes.
#[inline]
pub fn register_locations(module: &Bound<'_, PyModule>) -> PyResult<()> {
    locations::register(module)
}
