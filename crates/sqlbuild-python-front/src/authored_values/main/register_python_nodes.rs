//! Register the python nodes functions and classes.

use crate::authored_values::_helpers::python_nodes::normalize_hook_reads;
use crate::authored_values::_helpers::python_nodes::retry_delay_seconds;
use crate::authored_values::_helpers::python_nodes::retry_policy_exceptions;
use crate::authored_values::_helpers::python_nodes::validate_audit_case;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the python nodes functions and classes.
pub fn register_python_nodes(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(retry_policy_exceptions, module)?)?;
    module.add_function(wrap_pyfunction!(retry_delay_seconds, module)?)?;
    module.add_function(wrap_pyfunction!(validate_audit_case, module)?)?;
    module.add_function(wrap_pyfunction!(normalize_hook_reads, module)?)?;
    Ok(())
}
