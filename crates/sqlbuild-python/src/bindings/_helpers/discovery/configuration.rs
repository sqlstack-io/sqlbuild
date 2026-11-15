//! Project inputs holding only the project and local configuration.

use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::types::PyString;
use sqlbuild_host::entries::models::ExecutorHost;

use crate::bindings::_helpers::compile_driver::config_validation::{
    ProjectConfiguration, reject_invalid_connection_configuration,
    reject_invalid_project_configuration,
};
use crate::bindings::_helpers::discovery::imported_providers::discover_provider_classes;
use crate::bindings::_helpers::discovery::project_config::{
    load_local_config_file, load_project_config_file, warn_deprecated_settings,
};
use crate::bindings::_helpers::discovery::provider_secrets::SecretPolicy;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::within::within;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

/// `DiscoveredProjectInputs` with the loaded configuration, inside the project discovery lifecycle.
pub(crate) fn discover_project_configuration<'py>(
    py: Python<'py>,
    project_dir: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    configuration_inputs(py, project_dir, |_| Ok(()))
}

/// The validated configuration without a discovery lifecycle, which reading two files never needs.
pub(crate) fn discover_connection_configuration<'py>(
    py: Python<'py>,
    project_dir: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    loaded_configuration(py, project_dir, reject_invalid_connection_configuration)
}

/// The configuration with discovery's checks and warnings, and the provider classes, without importing any other project Python.
pub(crate) fn discover_configured_providers<'py>(
    py: Python<'py>,
    project_dir: &Bound<'py, PyAny>,
    secret_policy: SecretPolicy,
) -> PyResult<Bound<'py, PyAny>> {
    let configuration: Bound<'py, PyAny> = loaded_configuration(py, project_dir, |config| {
        reject_invalid_project_configuration(config)?;
        warn_deprecated_settings(py, project_dir)
    })?;
    let providers: Bound<'py, PyAny> =
        discover_provider_classes(project_dir.clone(), secret_policy.name())?;
    new_record_with(
        py,
        RecordId::DiscoveredProjectInputs,
        &[
            ("project_dir", project_dir.clone()),
            (
                "project_config",
                native_field(&configuration, "project_config")?,
            ),
            (
                "local_config",
                native_field(&configuration, "local_config")?,
            ),
            ("providers", providers),
        ],
    )
}

fn configuration_inputs<'py>(
    py: Python<'py>,
    project_dir: &Bound<'py, PyAny>,
    checks: impl FnOnce(&ProjectConfiguration<'_, '_>) -> PyResult<()>,
) -> PyResult<Bound<'py, PyAny>> {
    let lifecycle: Bound<'py, PyAny> = call_with(
        &host_entry(py, ExecutorHost::OperationLifecycle)?,
        &[
            ("operation_kind", PyString::new(py, "project").into_any()),
            (
                "operation_name",
                PyString::new(py, "project_discovery").into_any(),
            ),
        ],
    )?;
    within(&lifecycle, |_| {
        loaded_configuration(py, project_dir, checks)
    })
}

/// Both configuration files, after `checks` accepts them.
fn loaded_configuration<'py>(
    py: Python<'py>,
    project_dir: &Bound<'py, PyAny>,
    checks: impl FnOnce(&ProjectConfiguration<'_, '_>) -> PyResult<()>,
) -> PyResult<Bound<'py, PyAny>> {
    let project_config: Bound<'py, PyAny> =
        load_project_config_file(py, project_dir)?.into_bound(py);
    let local_config: Bound<'py, PyAny> = load_local_config_file(py, project_dir)?.into_bound(py);
    checks(&ProjectConfiguration {
        project_dir,
        project_config: &project_config,
        local_config: &local_config,
    })?;
    new_record_with(
        py,
        RecordId::DiscoveredProjectInputs,
        &[
            ("project_dir", project_dir.clone()),
            ("project_config", project_config),
            ("local_config", local_config),
        ],
    )
}
