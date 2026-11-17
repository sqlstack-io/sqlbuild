//! The effective connection, settings and vars of a project, its local config and target.

use crate::compile_attachment::main::build_effective_vars::build_effective_vars;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::PyDict;

use sqlbuild_model_config::templates::models::TemplateOptions;

use crate::contracts::main::effective_connection_fields::effective_connection_fields;
use crate::model_config::main::config::expanded_template_data::expanded_template_data;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;

const CONNECTION_LABEL: &str = "effective connection";

/// Expand only the connection selected for the caller's writer or catalog role.
pub(crate) fn expanded_connection<'py>(
    (project_config, local_config): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    target_config: &Bound<'py, PyAny>,
    effective_vars: &Bound<'py, PyAny>,
    catalog: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project_config.py();
    let (connection, error): (Bound<'py, PyAny>, Option<String>) =
        effective_connection_fields(py, (project_config, local_config), target_config, catalog)
            .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?
            .extract()?;
    if connection.is_none() {
        let raised: Bound<'py, PyAny> =
            error_class(py, ErrorId::CompileInput)?.call1((error.unwrap_or_default(),))?;
        return Err(PyErr::from_value(raised));
    }
    expanded_template_data(
        &connection,
        (effective_vars.downcast()?, &PyDict::new(py)),
        (
            TemplateOptions {
                allow_context: false,
                preserve_context_tokens: false,
                preserve_unknown_context: false,
            },
            CONNECTION_LABEL,
        ),
    )
}

/// The effective vars of a discovered project for `target_config` and optional CLI vars.
pub(crate) fn discovered_vars<'py>(
    discovered: &Bound<'py, PyAny>,
    target_config: &Bound<'py, PyAny>,
    cli_vars: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = discovered.py();
    let cli_vars: Bound<'py, PyAny> = match cli_vars {
        Some(cli_vars) if !cli_vars.is_none() => cli_vars,
        _ => PyDict::new(py).into_any(),
    };
    build_effective_vars(
        &native_field(discovered, "project_config")?,
        &native_field(discovered, "local_config")?,
        target_config,
        &cli_vars,
    )
}
