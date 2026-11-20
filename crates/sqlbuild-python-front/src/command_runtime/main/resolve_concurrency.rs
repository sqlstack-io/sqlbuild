//! The effective concurrency of a command on `selected_target`.

use crate::command_runtime::main::resolve_configured_concurrency::resolve_configured_concurrency;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_config::effective::models::EffectiveConcurrency;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The effective concurrency of a command on `selected_target`.
pub fn resolve_concurrency(
    requested: &Bound<'_, PyAny>,
    discovered: &Bound<'_, PyAny>,
    selected_target: &Bound<'_, PyAny>,
) -> PyResult<EffectiveConcurrency> {
    resolve_configured_concurrency(
        requested,
        (
            &native_field(discovered, "project_config")?,
            &native_field(discovered, "local_config")?,
        ),
        selected_target,
    )
}
