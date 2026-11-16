//! Template expansion for compile inputs and effective vars, with the reads it makes recorded.

use std::time::{SystemTime, UNIX_EPOCH};

use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyDict, PyDictMethods, PyString};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::scenario_command::main::format_captured_at::format_captured_at;
use sqlbuild_model_config::templates::models::TemplateOptions;

use crate::bindings::_helpers::model_config::config_errors::config_exception;
use crate::bindings::_helpers::model_config::config_templates::{
    TemplateOutcome, TemplateSources, expand_config_templates, lone_surrogate_site,
    resolve_effective_vars,
};
use crate::bindings::_helpers::model_config::unicode_environment::unicode_environment;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::boundary::models::Request;
use sqlbuild_python_base::compile_context::main::input_reads::context_read::context_read;
use sqlbuild_python_base::compile_context::main::input_reads::environment_read::environment_read;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::types::ErrorId;

const UUID_MODULE: &str = "uuid";
const ENVIRONMENT_READ: &str = "env";
const RUN_ID_KEY: &str = "run.id";
const RUN_TARGET_KEY: &str = "run.target";
const MODEL_CONFIG_LABEL: &str = "model config";
const RUN_ID_SUFFIX_LENGTH: usize = 12;
const TIMESTAMP_SEPARATORS: [char; 2] = ['-', ':'];

/// Record native template environment and context reads for project reuse, in order.
pub(crate) fn record_reads<'a>(
    py: Python<'_>,
    reads: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> PyResult<()> {
    for (kind, name) in reads {
        if kind == ENVIRONMENT_READ {
            environment_read(py, name)?;
        } else {
            context_read(py, name)?;
        }
    }
    Ok(())
}

/// Record `outcome`'s reads, then return its value or raise its error as Python's stage does.
fn applied<'py>(py: Python<'py>, outcome: TemplateOutcome<'py>) -> PyResult<Bound<'py, PyAny>> {
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
fn environment(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    unicode_environment(py)
}

/// `value` with every `${...}` template expanded, recording the reads it made.
pub(crate) fn expanded_template_data<'py>(
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

/// Merged effective vars with recursive `${name}` expansion.
pub(crate) fn expanded_effective_vars<'py>(
    raw_values: &Bound<'py, PyDict>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = raw_values.py();
    if let Some((name, path, byte_offset)) = lone_surrogate_site(raw_values)? {
        let error: Bound<'py, PyAny> = error_class(py, ErrorId::CompileInput)?.call(
            (format!(
                "Variable '{name}' holds a lone surrogate in {path} at UTF-8 byte \
                     {byte_offset}, which is not valid Unicode text"
            ),),
            Some(&keywords(
                py,
                &[(
                    "help",
                    PyString::new(
                        py,
                        &format!(
                            "Set '{name}' to valid Unicode text in --vars or the [vars] \
                                 table; the value is not shown because it may be a secret"
                        ),
                    )
                    .into_any(),
                )],
            )?),
        )?;
        return Err(PyErr::from_value(error));
    }
    applied(
        py,
        resolve_effective_vars(py, raw_values, environment(py)?)?,
    )
}

/// Model header `values` with `${name}`, `${ENV:...}` and early `run.*` templates resolved.
pub(crate) fn early_model_templates<'py>(
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
        (effective_vars.downcast()?, &context_values),
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

/// The selected run id, or a new `YYYYMMDDTHHMMSSZ_<12 hex>` compile invocation id.
pub(crate) fn run_id<'py>(selected_run_id: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = selected_run_id.py();
    if !selected_run_id.is_none() {
        return Ok(selected_run_id.clone());
    }
    let epoch_seconds: i64 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX)
        });
    let timestamp: String = format_captured_at(epoch_seconds).replace(TIMESTAMP_SEPARATORS, "");
    let suffix: String = imported(py, UUID_MODULE, "uuid4")?
        .call0()?
        .getattr("hex")?
        .extract()?;
    let suffix: &str = suffix.get(..RUN_ID_SUFFIX_LENGTH).unwrap_or(&suffix);
    Ok(PyString::new(py, &format!("{timestamp}_{suffix}")).into_any())
}

/// Recursively expand template strings inside supported Python container values.
#[pyfunction]
#[pyo3(signature = (**request))]
fn expand_template_data<'py>(request: Option<&Bound<'py, PyDict>>) -> PyResult<Bound<'py, PyAny>> {
    let request: Request<'_, 'py> = Request::new("expand_template_data", request)?;
    let flag = |name: &str| -> PyResult<bool> { request.get(name)?.is_truthy() };
    expanded_template_data(
        &request.get("value")?,
        (
            request.get("variables")?.downcast()?,
            request.get("context_values")?.downcast()?,
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
fn expand_effective_vars<'py>(raw_values: &Bound<'py, PyDict>) -> PyResult<Bound<'py, PyAny>> {
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
fn resolve_run_id<'py>(selected_run_id: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    run_id(selected_run_id)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(expand_template_data, module)?)?;
    module.add_function(wrap_pyfunction!(expand_effective_vars, module)?)?;
    module.add_function(wrap_pyfunction!(record_template_reads, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_run_id, module)?)?;
    Ok(())
}
