//! `value` with every `${...}` template expanded, recording the reads it made.

use crate::model_config::_helpers::config::config_templates::expand_config_templates;
use crate::model_config::_helpers::config::template_data::applied;
use crate::model_config::_helpers::config::template_data::environment;
use crate::model_config::models::TemplateSources;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::types::PyDict;
use sqlbuild_model_config::templates::models::TemplateOptions;

/// `value` with every `${...}` template expanded, recording the reads it made.
pub fn expanded_template_data<'py>(
    value: &Bound<'py, PyAny>,
    (variables, context_values): (&Bound<'py, PyDict>, &Bound<'py, PyDict>),
    (options, label): (TemplateOptions, &str),
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = value.py();
    let sources = TemplateSources(variables.clone(), environment(py)?, context_values.clone());
    applied(
        py,
        expand_config_templates(py, value, sources, (options, label))?,
    )
}
