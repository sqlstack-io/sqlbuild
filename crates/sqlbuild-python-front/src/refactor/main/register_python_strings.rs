//! Register the python strings functions and classes.

use crate::refactor::_helpers::python_strings::find_python_string_locations;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the python strings functions and classes.
pub fn register_python_strings(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(find_python_string_locations, module)?)?;
    Ok(())
}
