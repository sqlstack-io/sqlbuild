//! Project variable values rendered as the text SQL and config interpolation inserts.

use sqlbuild_host::entries::models::FacadeHost;
use sqlbuild_python_base::native_host::main::host_error::host_error;

use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyBool, PyDict, PyFloat, PyInt, PyList, PyString, PyTypeMethods};
use pyo3::{pyfunction, wrap_pyfunction};

const JSON_MODULE: &str = "json";
/// Characters of a structured value's JSON preview before it is cut short.
const PREVIEW_LIMIT: usize = 120;
const PREVIEW_ELLIPSIS: &str = "...";

/// `SharedInputError(message)`.
fn shared_input_error(py: Python<'_>, message: String) -> PyErr {
    host_error(py, FacadeHost::SharedInputError, &message)
}

/// Compact, key-sorted JSON of `value`, cut to the preview limit.
fn preview_json(value: &Bound<'_, PyAny>) -> PyResult<String> {
    let py: Python<'_> = value.py();
    let keywords: Bound<'_, PyDict> = PyDict::new(py);
    keywords.set_item("separators", (",", ":"))?;
    keywords.set_item("sort_keys", true)?;
    let preview: String = py
        .import(JSON_MODULE)?
        .getattr("dumps")?
        .call((value,), Some(&keywords))?
        .extract()?;
    let characters: Vec<char> = preview.chars().collect();
    if characters.len() <= PREVIEW_LIMIT {
        return Ok(preview);
    }
    let kept: String = characters[..PREVIEW_LIMIT - PREVIEW_ELLIPSIS.len()]
        .iter()
        .collect();
    Ok(format!("{kept}{PREVIEW_ELLIPSIS}"))
}

/// One project var value as interpolated text; structured values are rejected.
#[pyfunction]
#[pyo3(signature = (*, value, label))]
pub(crate) fn render_project_var_text(value: &Bound<'_, PyAny>, label: &str) -> PyResult<String> {
    project_var_text(value, label)
}

/// One project var value as interpolated text; structured values are rejected.
pub(crate) fn project_var_text(value: &Bound<'_, PyAny>, label: &str) -> PyResult<String> {
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

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(render_project_var_text, module)?)?;
    Ok(())
}
