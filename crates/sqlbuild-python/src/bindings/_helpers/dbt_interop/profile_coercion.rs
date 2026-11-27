//! dbt profile `as_bool` and `as_number` Jinja filters.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyBool, PyFloat, PyInt, PyString, PyStringMethods};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_config::dbt_profile::main::dbt_bool_text::dbt_bool_text;
use sqlbuild_config::dbt_profile::main::dbt_number_text::dbt_number_text;
use sqlbuild_config::dbt_profile::models::DbtNumberParse;

use crate::bindings::_helpers::dbt_interop::profile_connection::profile_error;

/// A `DbtProfileError` for a value that does not coerce to `target`.
fn coercion_error(value: &Bound<'_, PyAny>, target: &str) -> PyResult<PyErr> {
    let rendered: String = value.repr()?.to_str()?.to_owned();
    Ok(profile_error(
        value.py(),
        format!("Cannot coerce {rendered} to {target}"),
    ))
}

/// A bool, or a string dbt reads as one.
#[pyfunction]
#[pyo3(signature = (value))]
fn coerce_dbt_profile_bool<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    if value.is_instance_of::<PyBool>() {
        return Ok(value.clone());
    }
    if let Ok(text) = value.cast::<PyString>()
        && let Some(flag) = dbt_bool_text(text.to_str()?)
    {
        return Ok(PyBool::new(value.py(), flag).to_owned().into_any());
    }
    Err(coercion_error(value, "bool")?)
}

/// An int or float, or a string parsed as one.
#[pyfunction]
#[pyo3(signature = (value))]
fn coerce_dbt_profile_number<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = value.py();
    if value.is_instance_of::<PyBool>() {
        return Err(coercion_error(value, "number")?);
    }
    if value.is_instance_of::<PyInt>() || value.is_instance_of::<PyFloat>() {
        return Ok(value.clone());
    }
    let Ok(text) = value.cast::<PyString>() else {
        return Err(coercion_error(value, "number")?);
    };
    let (stripped, parse): (&str, DbtNumberParse) = dbt_number_text(text.to_str()?);
    let parsed: PyResult<Bound<'py, PyAny>> = match parse {
        DbtNumberParse::Float => py.get_type::<PyFloat>().call1((stripped,)),
        DbtNumberParse::Integer => py.get_type::<PyInt>().call1((stripped,)),
    };
    match parsed {
        Err(error) if error.is_instance_of::<PyValueError>(py) => {
            let raised: PyErr = coercion_error(value, "number")?;
            raised.set_cause(py, Some(error));
            Err(raised)
        }
        parsed => parsed,
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(coerce_dbt_profile_bool, module)?)?;
    module.add_function(wrap_pyfunction!(coerce_dbt_profile_number, module)?)
}
