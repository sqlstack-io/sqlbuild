//! Register the fields functions and classes.

use crate::authored_values::_helpers::fields::optional_named_bool;
use crate::authored_values::_helpers::fields::optional_named_string;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the fields functions and classes.
pub fn register_fields(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(optional_named_string, module)?)?;
    module.add_function(wrap_pyfunction!(optional_named_bool, module)?)?;
    Ok(())
}
