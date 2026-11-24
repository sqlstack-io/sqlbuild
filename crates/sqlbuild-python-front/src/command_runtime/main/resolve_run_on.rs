//! Where a command runs its unit tests or scenarios: the flag, the target, then `[settings]`.

use crate::command_runtime::main::target_and_settings_value::target_and_settings_value;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_core::run_on::main::resolve_run_on::resolve_run_on as resolve_layers;
use sqlbuild_core::run_on::models::{RunOn, RunOnLayers};

/// The effective `key` (`test_run_on` or `scenario_run_on`) of a command on `selected_target`.
pub fn resolve_run_on(
    flag: &Bound<'_, PyAny>,
    discovered: &Bound<'_, PyAny>,
    selected_target: &Bound<'_, PyAny>,
    key: &str,
) -> PyResult<RunOn> {
    let (target, settings) = target_and_settings_value(discovered, selected_target, key)?;
    Ok(resolve_layers(&RunOnLayers {
        flag: flag.extract()?,
        target: target.map(|value| value.extract()).transpose()?.flatten(),
        settings: settings.extract()?,
    }))
}
