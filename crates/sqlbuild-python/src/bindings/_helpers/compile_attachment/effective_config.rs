//! The effective connection, settings and vars of a project, its local config and target.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyString, PyTuple};

use sqlbuild_model_config::templates::models::TemplateOptions;

use crate::bindings::_helpers::boundary::python_calls::imported;
use crate::bindings::_helpers::contracts::effective_config::effective_config_connection;
use crate::bindings::_helpers::contracts::effective_config::effective_config_settings;
use crate::bindings::_helpers::contracts::effective_config::effective_config_vars;
use crate::bindings::_helpers::contracts::spec_resolution::{
    resolve_target_config, resolve_target_name,
};
use crate::bindings::_helpers::model_config::template_data::{
    expanded_effective_vars, expanded_template_data, run_id,
};
use crate::bindings::_helpers::records::construct::error_class;
use crate::bindings::_helpers::records::construct::{new_record, record_class};
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::{ErrorId, RecordId};

const CONNECTION_LABEL: &str = "effective connection";

/// Merge base project connection with the selected environment overrides.
#[pyfunction]
#[pyo3(signature = (*, project_config, local_config, target_config, effective_vars))]
pub(crate) fn build_effective_connection<'py>(
    project_config: &Bound<'py, PyAny>,
    local_config: &Bound<'py, PyAny>,
    target_config: &Bound<'py, PyAny>,
    effective_vars: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project_config.py();
    let (connection, error): (Bound<'py, PyAny>, Option<String>) =
        effective_config_connection(py, project_config, local_config, target_config)
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

/// Merge project settings with local developer overrides.
#[pyfunction]
#[pyo3(signature = (*, project_config, local_config))]
pub(crate) fn build_effective_settings<'py>(
    project_config: &Bound<'py, PyAny>,
    local_config: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project_config.py();
    let settings_class: Bound<'py, PyAny> = record_class(py, RecordId::SettingsConfig)?;
    let names: Vec<Bound<'py, PyAny>> = imported(py, "dataclasses", "fields")?
        .call1((&settings_class,))?
        .try_iter()?
        .map(|field| native_field(&field?, "name"))
        .collect::<PyResult<_>>()?;
    let values: Bound<'py, PyDict> = effective_config_settings(
        py,
        (
            native_field(project_config, "settings")?,
            native_field(local_config, "settings")?,
        ),
        &native_field(local_config, "setting_overrides")?,
        names
            .into_iter()
            .map(|name| Ok(name.cast_into::<PyString>()?))
            .collect::<PyResult<_>>()?,
    )
    .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?
    .downcast_into()?;
    new_record(py, RecordId::SettingsConfig, (), Some(&values))
}

/// Merge effective vars using the locked precedence order.
#[pyfunction]
#[pyo3(signature = (*, project_config, local_config, target_config, cli_vars))]
pub(crate) fn build_effective_vars<'py>(
    project_config: &Bound<'py, PyAny>,
    local_config: &Bound<'py, PyAny>,
    target_config: &Bound<'py, PyAny>,
    cli_vars: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project_config.py();
    let merged: Bound<'py, PyAny> = effective_config_vars(
        py,
        (project_config.clone(), local_config.clone()),
        target_config,
        cli_vars,
    )
    .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?;
    expanded_effective_vars(merged.downcast()?)
}

/// The selected target name of a discovered project and its effective config, if any.
pub(crate) fn target_selection<'py>(
    discovered: &Bound<'py, PyAny>,
    selected: Option<Bound<'py, PyAny>>,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    let py: Python<'py> = discovered.py();
    let project_config: Bound<'py, PyAny> = native_field(discovered, "project_config")?;
    let local_config: Bound<'py, PyAny> = native_field(discovered, "local_config")?;
    let selected: Bound<'py, PyAny> = selected.unwrap_or_else(|| py.None().into_bound(py));
    let Some(name) = resolve_target_name(py, &project_config, &local_config, &selected)? else {
        return Ok((py.None().into_bound(py), py.None().into_bound(py)));
    };
    let name: Bound<'py, PyAny> = name.into_any();
    let config: Bound<'py, PyAny> =
        resolve_target_config(py, &project_config, &local_config, &name)?;
    Ok((name, config))
}

/// The effective vars of a discovered project for `target_config` and optional CLI vars.
fn discovered_vars<'py>(
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

/// Build effective target config without compiling resources.
#[pyfunction]
#[pyo3(signature = (*, discovered_inputs, selected_target = None))]
pub(crate) fn build_effective_target_config<'py>(
    discovered_inputs: &Bound<'py, PyAny>,
    selected_target: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    Ok(target_selection(discovered_inputs, selected_target)?.1)
}

/// Build the effective project connection config without compiling resources.
#[pyfunction]
#[pyo3(signature = (*, discovered_inputs, selected_target = None, cli_vars = None))]
pub(crate) fn build_effective_connection_config<'py>(
    discovered_inputs: &Bound<'py, PyAny>,
    selected_target: Option<Bound<'py, PyAny>>,
    cli_vars: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let (_, target_config) = target_selection(discovered_inputs, selected_target)?;
    let effective_vars: Bound<'py, PyAny> =
        discovered_vars(discovered_inputs, &target_config, cli_vars)?;
    build_effective_connection(
        &native_field(discovered_inputs, "project_config")?,
        &native_field(discovered_inputs, "local_config")?,
        &target_config,
        &effective_vars,
    )
}

/// Build environment, vars and invocation id without compiling resources.
#[pyfunction]
#[pyo3(signature = (*, discovered_inputs, selected_target = None, cli_vars = None))]
pub(crate) fn build_effective_runtime_config<'py>(
    discovered_inputs: &Bound<'py, PyAny>,
    selected_target: Option<Bound<'py, PyAny>>,
    cli_vars: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = discovered_inputs.py();
    let (target_name, target_config) = target_selection(discovered_inputs, selected_target)?;
    let effective_vars: Bound<'py, PyAny> =
        discovered_vars(discovered_inputs, &target_config, cli_vars)?;
    let run_id: Bound<'py, PyAny> = run_id(&py.None().into_bound(py))?;
    PyTuple::new(py, [target_name, effective_vars, run_id])
}

/// Build effective settings without compiling resources.
#[pyfunction]
#[pyo3(signature = (*, discovered_inputs))]
pub(crate) fn build_effective_settings_config<'py>(
    discovered_inputs: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    build_effective_settings(
        &native_field(discovered_inputs, "project_config")?,
        &native_field(discovered_inputs, "local_config")?,
    )
}

/// `build_effective_vars` for a discovered project and target, without CLI vars.
pub(crate) fn project_vars<'py>(
    discovered: &Bound<'py, PyAny>,
    target_config: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    build_effective_vars(
        &native_field(discovered, "project_config")?,
        &native_field(discovered, "local_config")?,
        target_config,
        PyDict::new(discovered.py()).as_any(),
    )
}
