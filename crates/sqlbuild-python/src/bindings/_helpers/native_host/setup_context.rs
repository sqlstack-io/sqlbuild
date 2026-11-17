//! The `ctx` a provider's `setup` receives outside a command's own session.

use pyo3::exceptions::PyException;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::boundary::main::within::within;
use sqlbuild_python_base::native_host::main::redirect_stdout_to_stderr::redirect_stdout_to_stderr;
use sqlbuild_python_compile::compile_driver::main::setup::selected_target_settings::selected_target_settings;
use sqlbuild_python_front::compile_attachment::main::build_effective_runtime_config::build_effective_runtime_config;

use crate::bindings::_helpers::command_runtime::connection::project_connection_config;
use crate::bindings::_helpers::query_command::project::{ProjectInputs, project_adapter};

/// Commands that use the target, so their exporter and sink providers fail startup on an unresolvable target rather than get `None`.
const TARGET_COMMANDS: [&str; 12] = [
    "audit",
    "build",
    "check",
    "compile",
    "dag",
    "debug",
    "freshness",
    "load",
    "plan",
    "query",
    "seed",
    "test",
];

/// The setup context of the CLI invocation `args`'s exporter and sink providers: the settings of its `--target`, or of the default target.
pub(crate) fn invocation_setup_context<'py>(
    args: &Bound<'py, PyAny>,
    (project_dir, providers): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyAny>> {
    let command: Option<String> = args.getattr("command")?.extract()?;
    let required = TARGET_COMMANDS.contains(&command.as_deref().unwrap_or_default());
    let target_settings = (&args.getattr("target")?, &args.getattr("vars")?);
    project_setup_context((project_dir, providers), target_settings, required)
}

/// The settings of `target` (the default target when `None`) for the project at `project_dir`, as a setup context for its discovered `providers`, such as for an event exporter or a sensor; `None` when it has no providers. When the project can't resolve them, the error propagates if the target is `required`, and the context is `None` otherwise. Project code imported on the way writes to stderr, so machine-readable stdout stays valid.
pub(crate) fn project_setup_context<'py>(
    (project_dir, providers): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    (target, cli_vars): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    required: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let py = project_dir.py();
    if !providers.is_truthy()? {
        return Ok(py.None().into_bound(py));
    }
    let resolved = within(&redirect_stdout_to_stderr(py)?, |_| {
        let project = project_adapter(py, Some(project_dir.clone()), ProjectInputs::Configuration)?;
        let connection = project_connection_config(
            &project.discovered,
            (&project.project_dir, target),
            (cli_vars, false),
            false,
        )?;
        let vars = build_effective_runtime_config(
            &project.discovered,
            Some(target.clone()).filter(|value| !value.is_none()),
            Some(cli_vars.clone()).filter(|value| !value.is_none()),
        )?
        .get_item(1)?;
        let adapter_name: String = project.adapter.getattr("adapter_name")?.extract()?;
        let settings = (&connection, &vars, &project.adapter);
        selected_target_settings((&project.discovered, target), &adapter_name, settings)?
            .setup_context(py)
    });
    match resolved {
        Err(error) if !required && error.is_instance_of::<PyException>(py) => {
            Ok(py.None().into_bound(py))
        }
        context => context,
    }
}
