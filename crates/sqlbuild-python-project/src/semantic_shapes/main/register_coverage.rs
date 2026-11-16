//! Register the coverage functions and classes.

use crate::semantic_shapes::_helpers::coverage::semantic_coverage;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the coverage functions and classes.
pub fn register_coverage(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(semantic_coverage, module)?)
}
