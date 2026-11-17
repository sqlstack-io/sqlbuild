//! Register the sql values functions and classes.

use crate::authored_values::_helpers::sql_values::normalize_sql_value;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the sql values functions and classes.
pub fn register_sql_values(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(normalize_sql_value, module)?)
}
