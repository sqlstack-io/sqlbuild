//! A build's dynamic-pause runtime settings: the target's `pause_dynamic` and `[dynamic]`.

use crate::contracts::main::resolve_target_config::resolve_target_config;
use crate::contracts::main::resolve_target_name::resolve_target_name;
use crate::discovery::_helpers::project::project_config::configured;
use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_config::project::main::load_dynamic_config::load_dynamic_config;
use sqlbuild_config::project::models::DynamicConfig;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// `BuildRuntimeParams` fields for the request's target and flag, and the project's `[dynamic]`.
pub fn configured_dynamic<'py>(
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
                "pause_dynamic",
            )?
            .extract()?,
            None => None,
        };
    let config: DynamicConfig = configured(project_dir, load_dynamic_config)?;
    Ok(vec![
        (
            "pause_dynamic",
            paused.unwrap_or(false).into_bound_py_any(py)?,
        ),
        (
            "dynamic_heartbeat_grace_seconds",
            config.heartbeat_grace_seconds.into_bound_py_any(py)?,
        ),
        (
            "allow_ignore_dynamic_grace",
            config.allow_ignore_grace.into_bound_py_any(py)?,
        ),
        (
            "ignore_dynamic_grace",
            native_field(request, "ignore_dynamic_grace")?,
        ),
    ])
}
