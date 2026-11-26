//! One setting as the selected target sets it and as `[settings]` sets it.

use crate::compile_attachment::main::build_effective_settings::build_effective_settings;
use crate::contracts::main::resolve_target_config::resolve_target_config;
use crate::contracts::main::resolve_target_name::resolve_target_name;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// `key` on the selected target (`None` without one), and `key` in the effective `[settings]`.
pub fn target_and_settings_value<'py>(
    discovered: &Bound<'py, PyAny>,
    selected_target: &Bound<'py, PyAny>,
    key: &str,
) -> PyResult<(Option<Bound<'py, PyAny>>, Bound<'py, PyAny>)> {
    let py = discovered.py();
    let project_config: Bound<'py, PyAny> = native_field(discovered, "project_config")?;
    let local_config: Bound<'py, PyAny> = native_field(discovered, "local_config")?;
    let target: Option<Bound<'py, PyAny>> =
        match resolve_target_name(py, &project_config, &local_config, selected_target)? {
            Some(name) => Some(native_field(
                &resolve_target_config(py, &project_config, &local_config, name.as_any())?,
                key,
            )?),
            None => None,
        };
    let settings: Bound<'py, PyAny> = native_field(
        &build_effective_settings(&project_config, &local_config)?,
        key,
    )?;
    Ok((target, settings))
}
