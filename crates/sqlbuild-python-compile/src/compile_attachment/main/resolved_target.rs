//! The selected target's name and merged config, or `(None, None)`.

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_front::contracts::main::resolve_target_config::resolve_target_config;
use sqlbuild_python_front::contracts::main::resolve_target_name::resolve_target_name;

/// The selected target's name and merged config, or `(None, None)`.
pub fn resolved_target<'py>(
    discovered: &Bound<'py, PyAny>,
    selected_target: &Bound<'py, PyAny>,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    let py: Python<'py> = discovered.py();
    let project_config: Bound<'py, PyAny> = native_field(discovered, "project_config")?;
    let local_config: Bound<'py, PyAny> = native_field(discovered, "local_config")?;
    let target_name: Bound<'py, PyAny> = resolve_target_name(
        py,
        &project_config.clone(),
        &local_config.clone(),
        &selected_target.clone(),
    )
    .and_then(|value| value.into_bound_py_any(py))?;
    if target_name.is_none() {
        return Ok((target_name, py.None().into_bound(py)));
    }
    let target_config: Bound<'py, PyAny> =
        resolve_target_config(py, &project_config, &local_config, &target_name.clone())
            .and_then(|value| value.into_bound_py_any(py))?;
    Ok((target_name, target_config))
}
