//! Register the coverage functions and classes.

use crate::semantic_shapes::_helpers::coverage;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the coverage functions and classes.
#[inline]
pub fn register_coverage(module: &Bound<'_, PyModule>) -> PyResult<()> {
    coverage::register(module)
}
