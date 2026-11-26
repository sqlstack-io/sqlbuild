//! Template expansion for compile inputs and effective vars, with the reads it makes recorded.

use crate::model_config::main::config::expanded_effective_vars::expanded_effective_vars;
use crate::model_config::main::config::expanded_template_data::expanded_template_data;
use crate::model_config::main::config::record_reads::record_reads;
use crate::model_config::main::config::run_id::run_id;
use crate::model_config::main::model_nodes::unicode_environment::unicode_environment;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyDict;
use sqlbuild_model_config::templates::models::TemplateOptions;

use crate::model_config::_helpers::config::config_errors::config_exception;
use crate::model_config::_helpers::config::config_templates::TemplateOutcome;
use sqlbuild_python_base::boundary::models::Request;

pub(crate) const UUID_MODULE: &str = "uuid";
pub(crate) const ENVIRONMENT_READ: &str = "env";
pub(crate) const RUN_ID_KEY: &str = "run.id";
pub(crate) const RUN_TARGET_KEY: &str = "run.target";
pub(crate) const MODEL_CONFIG_LABEL: &str = "model config";
pub(crate) const RUN_ID_SUFFIX_LENGTH: usize = 12;
pub(crate) const TIMESTAMP_SEPARATORS: [char; 2] = ['-', ':'];

/// Record `outcome`'s reads, then return its value or raise its error as Python's stage does.
pub(crate) fn applied<'py>(
    py: Python<'py>,
    outcome: TemplateOutcome<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    let (value, reads) = outcome;
    record_reads(py, reads.iter().map(|(kind, name)| (*kind, name.as_str())))?;
    match value {
        Ok(value) => Ok(value),
        Err(error) => Err(PyErr::from_value(config_exception(
            error.bind(py).as_any(),
            None,
        )?)),
    }
}

/// The process environment as compile text reads it.
pub(crate) fn environment(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    unicode_environment(py)
}

/// Recursively expand template strings inside supported Python container values.
#[pyfunction]
#[pyo3(signature = (**request))]
pub(crate) fn expand_template_data<'py>(
    request: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let request: Request<'_, 'py> = Request::new("expand_template_data", request)?;
    let flag = |name: &str| -> PyResult<bool> { request.get(name)?.is_truthy() };
    expanded_template_data(
        &request.get("value")?,
        (
            request.get("variables")?.cast()?,
            request.get("context_values")?.cast()?,
        ),
        (
            TemplateOptions {
                allow_context: flag("allow_context")?,
                preserve_context_tokens: flag("preserve_context_tokens")?,
                preserve_unknown_context: flag("preserve_unknown_context")?,
            },
            &request.get("context_label")?.extract::<String>()?,
        ),
    )
}

/// Resolve merged effective vars with recursive `${name}` expansion.
#[pyfunction]
pub(crate) fn expand_effective_vars<'py>(
    raw_values: &Bound<'py, PyDict>,
) -> PyResult<Bound<'py, PyAny>> {
    expanded_effective_vars(raw_values)
}

/// Record native template environment and context reads for project reuse, in order.
#[pyfunction]
pub(crate) fn record_template_reads(py: Python<'_>, reads: Vec<(String, String)>) -> PyResult<()> {
    record_reads(
        py,
        reads
            .iter()
            .map(|(kind, name)| (kind.as_str(), name.as_str())),
    )
}

/// Resolve a stable compile invocation id.
#[pyfunction]
#[pyo3(signature = (*, selected_run_id))]
pub(crate) fn resolve_run_id<'py>(
    selected_run_id: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    run_id(selected_run_id)
}
