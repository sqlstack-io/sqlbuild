//! `key`'s cursor bound as text, or `None` when absent or `None`.

use crate::model_config::_helpers::config::typed_config::cursor_bound_object;
use crate::model_config::_helpers::config::typed_config::node;
use crate::model_config::_helpers::config::typed_config::type_error;
use crate::model_config::types::ConfigValues;
use crate::model_config::types::PyConfigNode;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_model_config::config_values::main::checked_cursor_bound::checked_cursor_bound;
use std::sync::Arc;

/// `key`'s cursor bound as text, or `None` when absent or `None`.
pub fn config_cursor_bound<'py>(
    py: Python<'py>,
    values: &ConfigValues<'_>,
    key: &str,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let node: Arc<PyConfigNode> = node(values, key)?;
    checked_cursor_bound(key, Some(&node))
        .map_err(|error| type_error(py, &error))?
        .map(|bound| cursor_bound_object(py, bound))
        .transpose()
}
