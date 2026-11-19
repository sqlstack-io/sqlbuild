//! A build's maintained-pause runtime settings: the target's `pause_maintained` and `[maintained]`.

use crate::contracts::main::resolve_target_config::resolve_target_config;
use crate::contracts::main::resolve_target_name::resolve_target_name;
use crate::discovery::_helpers::project::project_config::configured;
use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_config::project::main::load_maintained_config::load_maintained_config;
use sqlbuild_config::project::models::MaintainedConfig;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// `BuildRuntimeParams` fields for the request's target and flag, and the project's `[maintained]`.
pub fn configured_maintained<'py>(
    discovered: &Bound<'py, PyAny>,
    request: &Bound<'py, PyAny>,
    project_dir: &Bound<'py, PyAny>,
) -> PyResult<Vec<(&'static str, Bound<'py, PyAny>)>> {
    let py = discovered.py();
    let selected_target: Bound<'py, PyAny> = native_field(request, "selected_target")?;
    let project_config: Bound<'py, PyAny> = native_field(discovered, "project_config")?;
    let local_config: Bound<'py, PyAny> = native_field(discovered, "local_config")?;
    let paused: Option<bool> =
        match resolve_target_name(py, &project_config, &local_config, &selected_target)? {
            Some(name) => native_field(
                &resolve_target_config(py, &project_config, &local_config, name.as_any())?,
                "pause_maintained",
            )?
            .extract()?,
            None => None,
        };
    let config: MaintainedConfig = configured(project_dir, load_maintained_config)?;
    Ok(vec![
        (
            "pause_maintained",
            paused.unwrap_or(false).into_bound_py_any(py)?,
        ),
        (
            "maintained_heartbeat_grace_seconds",
            config.heartbeat_grace_seconds.into_bound_py_any(py)?,
        ),
        (
            "allow_ignore_maintained_grace",
            config.allow_ignore_grace.into_bound_py_any(py)?,
        ),
        (
            "ignore_maintained_grace",
            native_field(request, "ignore_maintained_grace")?,
        ),
    ])
}
