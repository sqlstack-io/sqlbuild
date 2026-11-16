//! dbt-compatible nullable model full-refresh semantics.

use crate::planner_phases::_helpers::rebuilds::full_refresh::configured_full_refresh;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::PyBool;

/// dbt-compatible nullable model full-refresh semantics.
pub fn resolve_model_full_refresh(
    model: &Bound<'_, PyAny>,
    cli_full_refresh: bool,
) -> PyResult<bool> {
    let configured = configured_full_refresh(model)?;
    if configured.is_instance_of::<PyBool>() {
        return configured.extract();
    }
    Ok(cli_full_refresh)
}
