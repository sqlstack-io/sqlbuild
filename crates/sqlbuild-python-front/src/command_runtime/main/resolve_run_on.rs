//! Where a command runs its unit tests or scenarios: the flag, the target, then `[settings]`.

use crate::compile_attachment::main::build_effective_settings::build_effective_settings;
use crate::contracts::main::resolve_target_config::resolve_target_config;
use crate::contracts::main::resolve_target_name::resolve_target_name;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_core::run_on::main::resolve_run_on::resolve_run_on as resolve_layers;
use sqlbuild_core::run_on::models::{RunOn, RunOnLayers};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The effective `key` (`test_run_on` or `scenario_run_on`) of a command on `selected_target`.
pub fn resolve_run_on(
    flag: &Bound<'_, PyAny>,
    discovered: &Bound<'_, PyAny>,
    selected_target: &Bound<'_, PyAny>,
    key: &str,
) -> PyResult<RunOn> {
    let py = discovered.py();
    let project_config: Bound<'_, PyAny> = native_field(discovered, "project_config")?;
    let local_config: Bound<'_, PyAny> = native_field(discovered, "local_config")?;
    let target: Option<String> =
        match resolve_target_name(py, &project_config, &local_config, selected_target)? {
            Some(name) => native_field(
                &resolve_target_config(py, &project_config, &local_config, name.as_any())?,
                key,
            )?
            .extract()?,
            None => None,
        };
    let settings: Option<String> = native_field(
        &build_effective_settings(&project_config, &local_config)?,
        key,
    )?
    .extract()?;
    Ok(resolve_layers(&RunOnLayers {
        flag: flag.extract()?,
        target,
        settings,
    }))
}
