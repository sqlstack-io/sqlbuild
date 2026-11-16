//! The project directory, its discovered inputs and its adapter.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};

use crate::bindings::_helpers::compile_driver::discovery::discover_project_inputs;
use crate::bindings::_helpers::contracts::spec_resolution::resolve_effective_adapter_name;
use crate::bindings::_helpers::discovery::configuration::discover_connection_configuration;
use crate::bindings::_helpers::discovery::provider_secrets::SecretPolicy;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::command_runtime::main::resolve_cli_adapter::resolve_cli_adapter;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The project directory, defaulting to the working directory, with its inputs and adapter.
pub(crate) struct ProjectAdapter<'py> {
    pub(crate) project_dir: Bound<'py, PyAny>,
    pub(crate) discovered: Bound<'py, PyAny>,
    pub(crate) adapter: Bound<'py, PyAny>,
}

/// How much of the project a command reads before it resolves the adapter.
#[derive(Clone, Copy)]
pub(crate) enum ProjectInputs {
    /// Every declaration file and the imported project Python.
    Discovered,
    /// As `Discovered`, leaving missing provider secrets to the runs that use the providers.
    DiscoveredDeferringSecrets,
    /// Only the project and local configuration, with its connection settings validated.
    Configuration,
}

pub(crate) fn project_adapter<'py>(
    py: Python<'py>,
    project_dir: Option<Bound<'py, PyAny>>,
    inputs: ProjectInputs,
) -> PyResult<ProjectAdapter<'py>> {
    let project_dir: Bound<'py, PyAny> = match project_dir.filter(|path| !path.is_none()) {
        Some(path) => path,
        None => imported(py, "pathlib", "Path")?.call_method0("cwd")?,
    };
    let discovered: Bound<'py, PyAny> = match inputs {
        ProjectInputs::Discovered => discover_project_inputs(
            py,
            Some(keywords(py, &[("project_dir", project_dir.clone())])?),
        )?,
        ProjectInputs::DiscoveredDeferringSecrets => discover_project_inputs(
            py,
            Some(keywords(
                py,
                &[
                    ("project_dir", project_dir.clone()),
                    (
                        "secret_policy",
                        pyo3::types::PyString::new(py, SecretPolicy::Defer.name()).into_any(),
                    ),
                ],
            )?),
        )?,
        ProjectInputs::Configuration => discover_connection_configuration(py, &project_dir)?,
    };
    let adapter_name: Bound<'py, PyAny> = resolve_effective_adapter_name(
        &native_field(&discovered, "project_config")?,
        &native_field(&discovered, "local_config")?,
    )?;
    let adapter: Bound<'py, PyAny> = resolve_cli_adapter(&adapter_name, &project_dir)?;
    Ok(ProjectAdapter {
        project_dir,
        discovered,
        adapter,
    })
}
