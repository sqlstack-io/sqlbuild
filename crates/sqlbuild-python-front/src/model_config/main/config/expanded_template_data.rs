//! `value` with every `${...}` template expanded, recording the reads it made.

use crate::model_config::_helpers::config::template_data;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyDict;
use sqlbuild_model_config::templates::models::TemplateOptions;

/// `value` with every `${...}` template expanded, recording the reads it made.
#[inline]
pub fn expanded_template_data<'py>(
    value: &Bound<'py, PyAny>,
    (variables, context_values): (&Bound<'py, PyDict>, &Bound<'py, PyDict>),
    (options, label): (TemplateOptions, &str),
) -> PyResult<Bound<'py, PyAny>> {
    template_data::expanded_template_data(value, (variables, context_values), (options, label))
}
