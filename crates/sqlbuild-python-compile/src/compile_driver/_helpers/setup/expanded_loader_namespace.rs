//! A managed source's `database` and `schema` with project vars expanded and no context templates.

use crate::compile_driver::_helpers::setup::targets::LOADER_NAMESPACE_LABEL;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::PyDict;
use sqlbuild_model_config::templates::models::TemplateOptions;
use sqlbuild_python_front::model_config::main::config::expanded_template_data::expanded_template_data;

/// A managed source's `database` and `schema` with project vars expanded and no context templates.
pub(crate) fn expanded_loader_namespace<'py>(
    value: &Bound<'py, PyAny>,
    effective_vars: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    expanded_template_data(
        value,
        (effective_vars.downcast()?, &PyDict::new(value.py())),
        (
            TemplateOptions {
                allow_context: false,
                preserve_context_tokens: false,
                preserve_unknown_context: false,
            },
            LOADER_NAMESPACE_LABEL,
        ),
    )
}
