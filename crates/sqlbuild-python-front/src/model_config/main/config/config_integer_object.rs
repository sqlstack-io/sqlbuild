//! `key`'s integer value as its Python object, or `None` when absent or `None`.

use crate::model_config::_helpers::config::typed_config::node;
use crate::model_config::_helpers::config::typed_config::type_error;
use crate::model_config::main::config::config_object::config_object;
use crate::model_config::types::ConfigValues;
use crate::model_config::types::PyConfigNode;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_model_config::config_values::main::checked_integer::checked_integer;
use std::sync::Arc;

/// `key`'s integer value as its Python object, or `None` when absent or `None`.
pub fn config_integer_object<'py>(
    py: Python<'py>,
    values: &ConfigValues<'_>,
    key: &str,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let node: Arc<PyConfigNode> = node(values, key)?;
    checked_integer(key, Some(&node))
        .map_err(|error| type_error(py, &error))?
        .map(|value| config_object(py, value))
        .transpose()
}
