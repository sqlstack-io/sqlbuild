//! Register the algorithms functions and classes.

use crate::graph::_helpers::nodes::algorithms;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the algorithms functions and classes.
#[inline]
pub fn register_algorithms(module: &Bound<'_, PyModule>) -> PyResult<()> {
    algorithms::register(module)
}
