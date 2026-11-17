//! Register the footer functions and classes.

use crate::build_progress::_helpers::footer::format_build_footer;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the footer functions and classes.
pub fn register_footer(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(format_build_footer, module)?)
}
