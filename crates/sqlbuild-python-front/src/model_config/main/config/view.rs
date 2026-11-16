//! A `CompileModelConfig` of `values` and the other `fields`.

use crate::model_config::_helpers::config::config_classes;
use crate::model_config::models::ConfigClasses;
use crate::model_config::types::BuiltConfigValues;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyDict;

/// A `CompileModelConfig` of `values` and the other `fields`.
#[inline]
pub fn view<'py>(
    classes: &ConfigClasses<'py>,
    values: &Bound<'py, PyDict>,
    fields: &Bound<'py, PyDict>,
    nodes: Option<&BuiltConfigValues>,
) -> PyResult<Bound<'py, PyAny>> {
    config_classes::view(classes, values, fields, nodes)
}
