//! One authored value as the Python header object discovery hands to config builds.

use crate::sqltext::_helpers::authored_values::constant_call;
use crate::sqltext::_helpers::authored_values::hook;
use crate::sqltext::_helpers::authored_values::instance;
use crate::sqltext::_helpers::authored_values::values_tuple;
use crate::sqltext::_helpers::authored_values::word_to_python;
use crate::sqltext::main::map_to_python::map_to_python;
use pyo3::prelude::{IntoPyObject, Py, PyAny, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyList};
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_sqltext::compiler::models::AuthoredValue;

/// One authored value as the Python header object discovery hands to config builds.
pub fn authored_value_to_python(py: Python<'_>, value: &AuthoredValue) -> PyResult<Py<PyAny>> {
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
