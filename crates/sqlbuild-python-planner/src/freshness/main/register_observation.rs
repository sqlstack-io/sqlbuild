//! Register the observation functions and classes.

use crate::freshness::_helpers::observation;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the observation functions and classes.
#[inline]
pub fn register_observation(module: &Bound<'_, PyModule>) -> PyResult<()> {
    observation::register(module)
}
