//! `resolve_concurrency` from the project and local configs themselves.

use crate::command_runtime::_helpers::concurrency::CONCURRENCY;
use crate::compile_attachment::main::build_effective_settings::build_effective_settings;
use crate::contracts::main::resolve_target_config::resolve_target_config;
use crate::contracts::main::resolve_target_name::resolve_target_name;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_config::effective::main::effective_concurrency::effective_concurrency;
use sqlbuild_config::effective::models::{ConcurrencyLayers, EffectiveConcurrency};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// `resolve_concurrency` from the project and local configs themselves.
pub fn resolve_configured_concurrency(
    requested: &Bound<'_, PyAny>,
    (project_config, local_config): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    selected_target: &Bound<'_, PyAny>,
) -> PyResult<EffectiveConcurrency> {
    let py = project_config.py();
    let target: Option<i64> =
        match resolve_target_name(py, project_config, local_config, selected_target)? {
            Some(name) => {
                let config: Bound<'_, PyAny> =
                    resolve_target_config(py, project_config, local_config, name.as_any())?;
                native_field(&config, CONCURRENCY)?.extract()?
            }
            None => None,
        };
    let settings: i64 = native_field(
        &build_effective_settings(project_config, local_config)?,
        CONCURRENCY,
    )?
    .extract()?;
    Ok(effective_concurrency(&ConcurrencyLayers {
        requested: requested.extract()?,
        target,
        settings,
    }))
}
