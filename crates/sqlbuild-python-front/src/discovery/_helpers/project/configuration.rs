//! Project inputs holding only the project and local configuration.

use crate::discovery::main::project::load_local_config_file::load_local_config_file;
use crate::discovery::main::project::load_project_config_file::load_project_config_file;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::types::PyString;
use sqlbuild_host::entries::models::ExecutorHost;

use crate::compile_driver::models::ProjectConfiguration;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::within::within;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::types::RecordId;

pub(crate) fn configuration_inputs<'py>(
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
pub(crate) fn loaded_configuration<'py>(
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
