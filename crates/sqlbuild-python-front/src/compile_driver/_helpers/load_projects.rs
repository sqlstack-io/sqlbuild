//! The SQLLoad project a load runs as, its connection's templates expanded as the target's are.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::types::PyDict;
use sqlbuild_load::loads::main::run_project_text::run_project_text;
use sqlbuild_load::loads::models::LoadTarget;
use sqlbuild_model_config::templates::models::TemplateOptions;
use sqlbuild_python_base::boundary::main::json_value_to_python::json_value_to_python;

use crate::compile_driver::_helpers::source_connections::{connection_roles, toml_value};
use crate::compile_driver::models::{ConnectionRoles, ProjectConfiguration};
use crate::model_config::main::config::expanded_template_data::expanded_template_data;

/// The session SQLLoad's target writes through: the target's own warehouse connection.
pub(crate) const WAREHOUSE_SESSION: &str = "sqlbuild_warehouse";
/// What a load's settings call the connection it reads.
const CONNECTION_KEY: &str = "connection";
/// How a template error names the value it was expanding.
const CONNECTION_LABEL: &str = "load connection";

/// The project text that runs the load into its target, reading only the connection it names.
pub(crate) fn load_project_text(
    config: &ProjectConfiguration<'_, '_>,
    effective_vars: &Bound<'_, PyDict>,
    (load, settings): (&str, &Bound<'_, PyAny>),
    target: &LoadTarget,
) -> PyResult<String> {
    let Some(toml::Value::Table(settings)) = toml_value(settings)? else {
        return Err(PyValueError::new_err(format!(
            "load '{load}' has no settings"
        )));
    };
    let named: Option<String> = match settings.get(CONNECTION_KEY) {
        Some(toml::Value::String(name)) => Some(name.clone()),
        _ => None,
    };
    let connections = named_connection(config, effective_vars, named.as_deref())?;
    run_project_text(Some((load, settings)), connections, target).map_err(PyValueError::new_err)
}

/// The source connection `name`, its templates expanded with the project's vars, as SQLLoad's `[connections]`; empty without one.
pub(crate) fn named_connection(
    config: &ProjectConfiguration<'_, '_>,
    effective_vars: &Bound<'_, PyDict>,
    name: Option<&str>,
) -> PyResult<toml::Table> {
    let py: Python<'_> = config.project_dir.py();
    let mut connections = toml::Table::new();
    let Some(name) = name else {
        return Ok(connections);
    };
    let roles: ConnectionRoles = connection_roles(config)?;
    let Some(connection) = roles.sources.into_iter().find(|source| source.name == name) else {
        return Ok(connections);
    };
    let authored: serde_json::Value = serde_json::to_value(connection.settings)
        .map_err(|error| PyValueError::new_err(error.to_string()))?;
    let expanded: Bound<'_, PyAny> = expanded_template_data(
        &json_value_to_python(py, &authored)?,
        (effective_vars, &PyDict::new(py)),
        (
            TemplateOptions {
                allow_context: false,
                preserve_context_tokens: false,
                preserve_unknown_context: false,
            },
            CONNECTION_LABEL,
        ),
    )?;
    if let Some(expanded) = toml_value(&expanded)? {
        connections.insert(name.to_owned(), expanded);
    }
    Ok(connections)
}
