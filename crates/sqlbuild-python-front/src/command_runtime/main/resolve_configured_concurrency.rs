//! `resolve_concurrency` from the project and local configs themselves.

use crate::command_runtime::_helpers::concurrency;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_config::effective::models::EffectiveConcurrency;

/// `resolve_concurrency` from the project and local configs themselves.
#[inline]
pub fn resolve_configured_concurrency(
    requested: &Bound<'_, PyAny>,
    (project_config, local_config): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    selected_target: &Bound<'_, PyAny>,
) -> PyResult<EffectiveConcurrency> {
    concurrency::resolve_configured_concurrency(
        requested,
        (project_config, local_config),
        selected_target,
    )
}
