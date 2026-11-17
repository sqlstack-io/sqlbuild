//! Authored header values as the Python header objects: scalars, sets, constants and hooks.

use crate::sqltext::main::authored_value_to_python::authored_value_to_python;
use crate::sqltext::main::map_to_python::map_to_python;
use pyo3::prelude::{IntoPyObject, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_sqltext::compiler::main::header_word_value::header_word_value;
use sqlbuild_sqltext::compiler::models::{AuthoredValue, HeaderWord};

pub(crate) fn word_to_python(py: Python<'_>, word: &str) -> PyResult<Py<PyAny>> {
    match header_word_value(word) {
        HeaderWord::Bool(value) => Ok(value.into_pyobject(py)?.to_owned().unbind().into_any()),
        HeaderWord::Null => Ok(py.None()),
        HeaderWord::Integer(digits) => match digits.parse::<i64>() {
            Ok(value) => Ok(value.into_pyobject(py)?.unbind().into_any()),
            Err(_) => Ok(py
                .import("builtins")?
                .getattr("int")?
                .call1((digits,))?
                .unbind()),
        },
        HeaderWord::Float(value) => Ok(value.into_pyobject(py)?.unbind().into_any()),
        HeaderWord::Text => Ok(word.into_pyobject(py)?.unbind().into_any()),
    }
}

pub(crate) fn instance(
    py: Python<'_>,
    class: RecordId,
    arguments: &pyo3::Bound<'_, PyDict>,
) -> PyResult<Py<PyAny>> {
    Ok(new_record(py, class, (), Some(arguments))?.unbind())
}

pub(crate) fn values_tuple(py: Python<'_>, values: &[AuthoredValue]) -> PyResult<Py<PyAny>> {
    let projected = values
        .iter()
        .map(|item| authored_value_to_python(py, item))
        .collect::<PyResult<Vec<_>>>()?;
    Ok(PyTuple::new(py, projected)?.unbind().into_any())
}

pub(crate) fn constant_call(
    py: Python<'_>,
    values: &[(String, AuthoredValue)],
) -> PyResult<Py<PyAny>> {
    let arguments = values
        .iter()
        .map(|(key, value)| {
            let value = authored_value_to_python(py, value)?;
            PyTuple::new(py, [key.into_pyobject(py)?.unbind().into_any(), value])
        })
        .collect::<PyResult<Vec<_>>>()?;
    let keywords = PyDict::new(py);
    keywords.set_item("arguments", PyTuple::new(py, arguments)?)?;
    instance(py, RecordId::AuthoredSqlValueCall, &keywords)
}

pub(crate) fn hook(
    py: Python<'_>,
    class: RecordId,
    name: &str,
    kwargs: &[(String, AuthoredValue)],
) -> PyResult<Py<PyAny>> {
    let keywords = PyDict::new(py);
    keywords.set_item("name", name)?;
    keywords.set_item("kwargs", map_to_python(py, kwargs)?)?;
    instance(py, class, &keywords)
}
