//! Model header `values` with `${name}`, `${ENV:...}` and early `run.*` templates resolved.

use crate::model_config::_helpers::config::template_data::MODEL_CONFIG_LABEL;
use crate::model_config::_helpers::config::template_data::RUN_ID_KEY;
use crate::model_config::_helpers::config::template_data::RUN_TARGET_KEY;
use crate::model_config::main::config::expanded_template_data::expanded_template_data;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_model_config::templates::models::TemplateOptions;

/// Model header `values` with `${name}`, `${ENV:...}` and early `run.*` templates resolved.
pub fn early_model_templates<'py>(
    values: &Bound<'py, PyAny>,
    effective_vars: &Bound<'py, PyAny>,
    (target_name, run_id): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = values.py();
    let context_values: Bound<'py, PyDict> = PyDict::new(py);
    context_values.set_item(RUN_ID_KEY, run_id)?;
    context_values.set_item(RUN_TARGET_KEY, target_name)?;
    expanded_template_data(
        values,
        (effective_vars.cast()?, &context_values),
        (
            TemplateOptions {
                allow_context: true,
                preserve_context_tokens: false,
                preserve_unknown_context: true,
            },
            MODEL_CONFIG_LABEL,
        ),
    )
}
