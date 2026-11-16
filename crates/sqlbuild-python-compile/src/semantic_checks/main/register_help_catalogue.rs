//! Register the help catalogue functions and classes.

use crate::semantic_checks::_helpers::recovery::help_catalogue::semantic_help_catalogue;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the help catalogue functions and classes.
pub fn register_help_catalogue(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(semantic_help_catalogue, module)?)
}
