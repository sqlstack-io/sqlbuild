//! A JSON value as Python builds it; arrays become tuples or lists.

use crate::scopes::_helpers::reports::scope_report_objects::PyObject;
use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyList, PyTuple};
use sqlbuild_core::json::models::JsonValue;

/// A JSON value as Python builds it; arrays become tuples or lists.
pub fn json_to_python(py: Python<'_>, value: JsonValue, tuples: bool) -> PyResult<PyObject> {
    match value {
        JsonValue::Null => Ok(py.None()),
        JsonValue::Bool(flag) => flag.into_py_any(py),
        JsonValue::Integer(integer) => match integer.to_i128() {
            Some(number) => number.into_py_any(py),
            None => Ok(py
                .import("builtins")?
                .getattr("int")?
                .call1((integer.as_str(),))?
                .unbind()),
        },
        JsonValue::Float(number) => number.into_py_any(py),
        JsonValue::String(text) => text.into_py_any(py),
        JsonValue::Utf16(units) => String::from_utf16_lossy(&units).into_py_any(py),
        JsonValue::Array(items) => {
            let values: Vec<PyObject> = items
                .into_iter()
                .map(|item| json_to_python(py, item, tuples))
                .collect::<PyResult<_>>()?;
            if tuples {
                PyTuple::new(py, values)?.into_py_any(py)
            } else {
                PyList::new(py, values)?.into_py_any(py)
            }
        }
        JsonValue::Object(entries) => {
            let dictionary: Bound<'_, PyDict> = PyDict::new(py);
            for (key, item) in entries {
                dictionary.set_item(key, json_to_python(py, item, tuples)?)?;
            }
            dictionary.into_py_any(py)
        }
    }
}
