//! The effective concurrency of a command on `selected_target`, and the layer that set it.

use crate::command_runtime::_helpers::concurrency;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_config::effective::models::EffectiveConcurrency;

/// The effective concurrency of a command on `selected_target`, and the layer that set it.
#[inline]
pub fn resolve_concurrency(
    requested: &Bound<'_, PyAny>,
    discovered: &Bound<'_, PyAny>,
    selected_target: &Bound<'_, PyAny>,
) -> PyResult<EffectiveConcurrency> {
    concurrency::resolve_concurrency(requested, discovered, selected_target)
}
