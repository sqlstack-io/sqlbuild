//! Register the project validation functions and classes.

use crate::discovery::_helpers::project::project_validation::validate_discovered_project_inputs;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the project validation functions and classes.
pub fn register_project_validation(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(
        validate_discovered_project_inputs,
        module
    )?)
}
