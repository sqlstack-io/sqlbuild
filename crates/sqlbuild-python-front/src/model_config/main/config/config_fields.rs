//! Every `CompileModelConfig` field but `values`, recording the build's template reads first.

use crate::model_config::_helpers::config::config_classes;
use crate::model_config::models::BuiltConfig;
use crate::model_config::models::ConfigClasses;
use pyo3::prelude::Bound;
use pyo3::prelude::PyResult;
use pyo3::types::PyDict;

/// Every `CompileModelConfig` field but `values`, recording the build's template reads first.
#[inline]
pub fn config_fields<'py>(
    classes: &ConfigClasses<'py>,
    built: &BuiltConfig<'py>,
    matched: Option<&str>,
) -> PyResult<Bound<'py, PyDict>> {
    config_classes::config_fields(classes, built, matched)
}
