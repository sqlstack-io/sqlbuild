//! The `ctx` a provider's `setup` receives outside a command's own session.

use pyo3::exceptions::PyException;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_python_compile::compile_driver::main::setup::selected_target_settings::selected_target_settings;
use sqlbuild_python_front::compile_attachment::main::build_effective_runtime_config::build_effective_runtime_config;

use crate::bindings::_helpers::command_runtime::connection::project_connection_config;
use crate::bindings::_helpers::query_command::project::{
    ProjectAdapter, ProjectInputs, project_adapter,
};

/// The settings of `target` (the default target when `None`) for the project at `project_dir`, as a provider setup context, such as for an event exporter or a sensor, or `None` when the project can't resolve them.
pub(crate) fn project_setup_context<'py>(
    project_dir: &Bound<'py, PyAny>,
    (target, cli_vars): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project_dir.py();
    let resolved = || -> PyResult<Bound<'py, PyAny>> {
        let ProjectAdapter {
            project_dir,
            discovered,
            adapter,
        } = project_adapter(py, Some(project_dir.clone()), ProjectInputs::Configuration)?;
        let connection: Bound<'py, PyAny> = project_connection_config(
            &discovered,
            (&project_dir, target),
            (cli_vars, false),
            false,
        )?;
        let (_, effective_vars, _): (Bound<'py, PyAny>, Bound<'py, PyAny>, Bound<'py, PyAny>) =
            build_effective_runtime_config(
                &discovered,
                Some(target.clone()).filter(|value| !value.is_none()),
                Some(cli_vars.clone()).filter(|value| !value.is_none()),
            )?
            .extract()?;
        selected_target_settings(
            (&discovered, target),
            &adapter.getattr("adapter_name")?.extract::<String>()?,
            (&connection, &effective_vars, &adapter),
        )?
        .setup_context(py)
    };
    match resolved() {
        Err(error) if error.is_instance_of::<PyException>(py) => Ok(py.None().into_bound(py)),
        context => context,
    }
}
