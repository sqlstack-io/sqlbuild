//! Expand every template in `value` and the containers inside it.

use crate::model_config::_helpers::config::config_templates;
use crate::model_config::models::PythonHost;
use crate::model_config::models::Stop;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use sqlbuild_model_config::templates::models::TemplateOptions;

/// Expand every template in `value` and the containers inside it.
#[inline]
pub fn expanded<'py>(
    host: &PythonHost<'py>,
    value: &Bound<'py, PyAny>,
    options: TemplateOptions,
) -> Result<Bound<'py, PyAny>, Stop> {
    config_templates::expanded(host, value, options)
}
