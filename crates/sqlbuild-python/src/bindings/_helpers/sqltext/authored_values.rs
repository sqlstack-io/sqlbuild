//! Authored header values as the Python header objects: scalars, sets, constants and hooks.

use pyo3::prelude::{IntoPyObject, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyList, PyTuple};
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_sqltext::compiler::main::header_word_value::header_word_value;
use sqlbuild_sqltext::compiler::models::{AuthoredValue, HeaderWord};

fn word_to_python(py: Python<'_>, word: &str) -> PyResult<Py<PyAny>> {
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

fn instance(
    py: Python<'_>,
    class: RecordId,
    arguments: &pyo3::Bound<'_, PyDict>,
) -> PyResult<Py<PyAny>> {
    Ok(new_record(py, class, (), Some(arguments))?.unbind())
}

fn values_tuple(py: Python<'_>, values: &[AuthoredValue]) -> PyResult<Py<PyAny>> {
    let projected = values
        .iter()
        .map(|item| authored_value_to_python(py, item))
        .collect::<PyResult<Vec<_>>>()?;
    Ok(PyTuple::new(py, projected)?.unbind().into_any())
}

fn constant_call(py: Python<'_>, values: &[(String, AuthoredValue)]) -> PyResult<Py<PyAny>> {
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

/// One authored value as the Python header object discovery hands to config builds.
pub(crate) fn authored_value_to_python(
    py: Python<'_>,
    value: &AuthoredValue,
) -> PyResult<Py<PyAny>> {
    match value {
        AuthoredValue::Null => Ok(py.None()),
        AuthoredValue::Boolean(value) => {
            Ok(value.into_pyobject(py)?.to_owned().unbind().into_any())
        }
        AuthoredValue::BareWord(value) => word_to_python(py, value),
        AuthoredValue::String(value) => Ok(value.into_pyobject(py)?.unbind().into_any()),
        AuthoredValue::List(values) => {
            let projected = values
                .iter()
                .map(|item| authored_value_to_python(py, item))
                .collect::<PyResult<Vec<_>>>()?;
            Ok(PyList::new(py, projected)?.unbind().into_any())
        }
        AuthoredValue::Map(values) => map_to_python(py, values),
        AuthoredValue::Set(values) => {
            let keywords = PyDict::new(py);
            keywords.set_item("values", values_tuple(py, values)?)?;
            instance(py, RecordId::AuthoredSqlSet, &keywords)
        }
        AuthoredValue::Tuple(values) => values_tuple(py, values),
        AuthoredValue::TypedConstant(values) => constant_call(py, values),
        AuthoredValue::InlineSqlHook(statement) => {
            let keywords = PyDict::new(py);
            keywords.set_item("statement", statement)?;
            instance(py, RecordId::SqlHookEntry, &keywords)
        }
        AuthoredValue::NamedSqlHook(name, kwargs) => {
            hook(py, RecordId::NamedSqlHookEntry, name, kwargs)
        }
        AuthoredValue::PythonHook(name, kwargs) => {
            hook(py, RecordId::PythonHookEntry, name, kwargs)
        }
    }
}

pub(crate) fn map_to_python(
    py: Python<'_>,
    values: &[(String, AuthoredValue)],
) -> PyResult<Py<PyAny>> {
    let result = PyDict::new(py);
    for (key, value) in values {
        result.set_item(key, authored_value_to_python(py, value)?)?;
    }
    Ok(result.unbind().into_any())
}

fn hook(
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

pub(crate) fn optional_authored_value_to_python(
    py: Python<'_>,
    value: Option<&AuthoredValue>,
) -> PyResult<Option<Py<PyAny>>> {
    value
        .map(|item| authored_value_to_python(py, item))
        .transpose()
}
