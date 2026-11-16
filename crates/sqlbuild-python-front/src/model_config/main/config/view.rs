//! A `CompileModelConfig` of `values` and the other `fields`.

use crate::model_config::_helpers::config::config_classes::VALUES_FIELD;
use crate::model_config::main::config::retain_values::retain_values;
use crate::model_config::models::ConfigClasses;
use crate::model_config::types::BuiltConfigValues;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods};

/// A `CompileModelConfig` of `values` and the other `fields`.
pub fn view<'py>(
    classes: &ConfigClasses<'py>,
    values: &Bound<'py, PyDict>,
    fields: &Bound<'py, PyDict>,
    nodes: Option<&BuiltConfigValues>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: Bound<'py, PyDict> = fields.copy()?;
    arguments.set_item(VALUES_FIELD, values)?;
    let config = classes.compile_model_config.call((), Some(&arguments))?;
    retain_values(&config, nodes)?;
    Ok(config)
}
