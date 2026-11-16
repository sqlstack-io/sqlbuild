//! `key`'s strings as a tuple, or `None` when absent or `None`.

use crate::model_config::_helpers::config::typed_config::node;
use crate::model_config::_helpers::config::typed_config::type_error;
use crate::model_config::main::config::config_object::config_object;
use crate::model_config::types::ConfigValues;
use crate::model_config::types::PyConfigNode;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::types::PyTuple;
use sqlbuild_model_config::config_values::main::checked_texts::checked_texts;
use std::sync::Arc;

/// `key`'s strings as a tuple, or `None` when absent or `None`.
pub fn config_text_tuple<'py>(
    py: Python<'py>,
    values: &ConfigValues<'_>,
    key: &str,
) -> PyResult<Option<Bound<'py, PyTuple>>> {
    let node: Arc<PyConfigNode> = node(values, key)?;
    let Some(items) = checked_texts(key, Some(&node)).map_err(|error| type_error(py, &error))?
    else {
        return Ok(None);
    };
    let objects: Vec<Bound<'py, PyAny>> = items
        .into_iter()
        .map(|item| config_object(py, item))
        .collect::<PyResult<_>>()?;
    PyTuple::new(py, objects).map(Some)
}
