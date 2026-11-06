//! Project inputs holding only the project and local configuration.

use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::types::PyString;
use sqlbuild_host::entries::models::ExecutorHost;

use crate::bindings::_helpers::boundary::python_calls::{call_with, within};
use crate::bindings::_helpers::compile_driver::config_validation::{
    ProjectConfiguration, reject_invalid_connection_configuration,
};
use crate::bindings::_helpers::discovery::project_config::{
    load_local_config_file, load_project_config_file,
};
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::records::construct::new_record_with;
use crate::bindings::_helpers::records::ids::RecordId;

/// `DiscoveredProjectInputs` with the loaded configuration, inside the project discovery lifecycle.
pub(crate) fn discover_project_configuration<'py>(
    py: Python<'py>,
    project_dir: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    configuration_inputs(py, project_dir, false)
}

/// The validated configuration without a discovery lifecycle, which reading two files never needs.
pub(crate) fn discover_connection_configuration<'py>(
    py: Python<'py>,
    project_dir: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    loaded_configuration(py, project_dir, true)
}

fn configuration_inputs<'py>(
    py: Python<'py>,
    project_dir: &Bound<'py, PyAny>,
    validate_connections: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let lifecycle: Bound<'py, PyAny> = call_with(
        &host::entry(py, ExecutorHost::OperationLifecycle)?,
        &[
            ("operation_kind", PyString::new(py, "project").into_any()),
            (
                "operation_name",
                PyString::new(py, "project_discovery").into_any(),
            ),
        ],
    )?;
    within(&lifecycle, |_| {
        loaded_configuration(py, project_dir, validate_connections)
    })
}

fn loaded_configuration<'py>(
    py: Python<'py>,
    project_dir: &Bound<'py, PyAny>,
    validate_connections: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let project_config: Bound<'py, PyAny> =
        load_project_config_file(py, project_dir)?.into_bound(py);
    let local_config: Bound<'py, PyAny> = load_local_config_file(py, project_dir)?.into_bound(py);
    if validate_connections {
        reject_invalid_connection_configuration(&ProjectConfiguration {
            project_dir,
            project_config: &project_config,
            local_config: &local_config,
        })?;
    }
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
