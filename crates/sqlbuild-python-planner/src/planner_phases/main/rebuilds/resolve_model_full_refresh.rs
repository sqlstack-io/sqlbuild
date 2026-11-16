//! dbt-compatible nullable model full-refresh semantics.

use crate::planner_phases::_helpers::rebuilds::full_refresh;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// dbt-compatible nullable model full-refresh semantics.
#[inline]
pub fn resolve_model_full_refresh(
    model: &Bound<'_, PyAny>,
    cli_full_refresh: bool,
) -> PyResult<bool> {
    full_refresh::resolve_model_full_refresh(model, cli_full_refresh)
}
