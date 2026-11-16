//! An authored string, preserving a present empty string.

use crate::model_config::_helpers::config::config_values::authored_node;
use crate::model_config::_helpers::config::typed_config::type_error;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::pyfunction;
use sqlbuild_model_config::config_values::main::checked_text::checked_text;

/// An authored string, preserving a present empty string.
#[pyfunction(name = "get_config_str")]
#[pyo3(signature = (*, values, key))]
pub fn config_str<'py>(
    values: &Bound<'py, PyAny>,
    key: &str,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let (node, value) = authored_node(values, key)?;
    let checked =
        checked_text(key, Some(&node)).map_err(|error| type_error(values.py(), &error))?;
    Ok(checked.map(|_| value))
}
