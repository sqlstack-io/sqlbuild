//! One project var value as interpolated text; structured values are rejected.

use crate::authored_values::_helpers::project_vars::preview_json;
use crate::authored_values::_helpers::project_vars::shared_input_error;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyBool, PyDict, PyFloat, PyInt, PyList, PyString, PyTypeMethods};

/// One project var value as interpolated text; structured values are rejected.
pub fn project_var_text(value: &Bound<'_, PyAny>, label: &str) -> PyResult<String> {
    let py: Python<'_> = value.py();
    if value.is_none() {
        return Ok(String::new());
    }
    if value.is_instance_of::<PyBool>() {
        return Ok(if value.is_truthy()? { "true" } else { "false" }.to_owned());
    }
    if value.is_instance_of::<PyString>()
        || value.is_instance_of::<PyInt>()
        || value.is_instance_of::<PyFloat>()
    {
        return value.str()?.extract();
    }
    let structure: Option<&str> = if value.is_instance_of::<PyDict>() {
        Some("an object")
    } else if value.is_instance_of::<PyList>() {
        Some("an array")
    } else {
        None
    };
    let message: String = match structure {
        Some(structure) => format!(
            "{label} is {structure} and cannot be interpolated as text: {}. Use a macro to \
             consume structured vars.",
            preview_json(value)?
        ),
        None => format!(
            "{label} has unsupported type {} and cannot be interpolated as text",
            value.get_type().name()?.repr()?
        ),
    };
    Err(shared_input_error(py, message))
}
