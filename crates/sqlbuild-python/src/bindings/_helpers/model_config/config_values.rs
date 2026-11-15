//! Typed values read from raw authored configuration mappings, rejecting the wrong types.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult};
use pyo3::types::{PyString, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_model_config::config_values::main::checked_cursor_bound::checked_cursor_bound;
use sqlbuild_model_config::config_values::main::checked_flag::checked_flag;
use sqlbuild_model_config::config_values::main::checked_integer::checked_integer;
use sqlbuild_model_config::config_values::main::checked_text::checked_text;
use sqlbuild_model_config::config_values::main::checked_texts::checked_texts;
use sqlbuild_model_config::config_values::models::CursorBound;

use crate::bindings::_helpers::model_config::config_nodes::{PyConfigNode, config_node};
use crate::bindings::_helpers::model_config::typed_config::type_error;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_base::records::types::ErrorId;

const ISOFORMAT_METHOD: &str = "isoformat";

/// The `ConfigValueTypeError` naming `key`, the `expected` kind and the value's `actual_type`.
pub(crate) fn config_value_type_error<'py>(
    key: &str,
    expected: &str,
    actual_type: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = actual_type.py();
    let type_name: String = actual_type.getattr("__name__")?.extract()?;
    call_with(
        &error_class(py, ErrorId::ConfigValueType)?,
        &[
            (
                "message",
                PyString::new(
                    py,
                    &format!("config key '{key}' expected {expected}, got {type_name}"),
                )
                .into_any(),
            ),
            ("key", PyString::new(py, key).into_any()),
            ("expected", PyString::new(py, expected).into_any()),
            ("actual_type", actual_type.clone()),
        ],
    )
}

/// `values.get(key)` as a config node.
fn authored_node<'py>(
    values: &Bound<'py, PyAny>,
    key: &str,
) -> PyResult<(PyConfigNode, Bound<'py, PyAny>)> {
    let value = mapping_get(values, key)?;
    Ok((config_node(&value)?, value))
}

/// An authored string, preserving a present empty string.
#[pyfunction(name = "get_config_str")]
#[pyo3(signature = (*, values, key))]
pub(crate) fn config_str<'py>(
    values: &Bound<'py, PyAny>,
    key: &str,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let (node, value) = authored_node(values, key)?;
    let checked =
        checked_text(key, Some(&node)).map_err(|error| type_error(values.py(), &error))?;
    Ok(checked.map(|_| value))
}

/// An authored boolean, or `None` when the key is absent or `None`.
#[pyfunction(name = "get_config_bool")]
#[pyo3(signature = (*, values, key))]
pub(crate) fn config_bool<'py>(
    values: &Bound<'py, PyAny>,
    key: &str,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let (node, value) = authored_node(values, key)?;
    let checked =
        checked_flag(key, Some(&node)).map_err(|error| type_error(values.py(), &error))?;
    Ok(checked.map(|_| value))
}

/// An authored sequence of strings as a tuple, or `None` when absent or `None`.
#[pyfunction(name = "get_config_string_tuple")]
#[pyo3(signature = (*, values, key))]
pub(crate) fn config_string_tuple<'py>(
    values: &Bound<'py, PyAny>,
    key: &str,
) -> PyResult<Option<Bound<'py, PyTuple>>> {
    let (node, value) = authored_node(values, key)?;
    let checked =
        checked_texts(key, Some(&node)).map_err(|error| type_error(values.py(), &error))?;
    if checked.is_none() {
        return Ok(None);
    }
    let items: Vec<Bound<'py, PyAny>> = value.try_iter()?.collect::<PyResult<_>>()?;
    PyTuple::new(values.py(), items).map(Some)
}

/// An authored integer, excluding booleans, or `None` when absent or `None`.
#[pyfunction(name = "get_config_int")]
#[pyo3(signature = (*, values, key))]
pub(crate) fn config_int<'py>(
    values: &Bound<'py, PyAny>,
    key: &str,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let (node, value) = authored_node(values, key)?;
    let checked =
        checked_integer(key, Some(&node)).map_err(|error| type_error(values.py(), &error))?;
    Ok(checked.map(|_| value))
}

/// A cursor bound as text from a string, integer, date or datetime.
#[pyfunction(name = "get_config_cursor_bound")]
#[pyo3(signature = (*, values, key))]
pub(crate) fn config_cursor_bound<'py>(
    values: &Bound<'py, PyAny>,
    key: &str,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let (node, value) = authored_node(values, key)?;
    let checked =
        checked_cursor_bound(key, Some(&node)).map_err(|error| type_error(values.py(), &error))?;
    Ok(match checked {
        None => None,
        Some(CursorBound::Dated(_)) => Some(value.call_method0(ISOFORMAT_METHOD)?),
        Some(CursorBound::Stringified(_)) => Some(value.str()?.into_any()),
    })
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(config_str, module)?)?;
    module.add_function(wrap_pyfunction!(config_bool, module)?)?;
    module.add_function(wrap_pyfunction!(config_string_tuple, module)?)?;
    module.add_function(wrap_pyfunction!(config_int, module)?)?;
    module.add_function(wrap_pyfunction!(config_cursor_bound, module)?)?;
    Ok(())
}
